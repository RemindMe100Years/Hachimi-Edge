use std::sync::Mutex;
use fnv::FnvHashMap;
use once_cell::sync::Lazy;
use crate::core::sugoi_client::SugoiClient;
use crate::il2cpp::{ext::{Il2CppStringExt, StringExt}, hook::UnityEngine_CoreModule::Object, symbols::get_method_addr, types::*};

pub static ACTIVE_TEXT_MESH_COMPONENTS: Lazy<Mutex<FnvHashMap<usize, (u32, String)>>> = Lazy::new(|| {
    Mutex::new(FnvHashMap::default())
});

static mut GET_TEXT_ADDR: usize = 0;
impl_addr_wrapper_fn!(get_text, GET_TEXT_ADDR, *mut Il2CppString, this: *mut Il2CppObject);

type SetTextFn = extern "C" fn(this: *mut Il2CppObject, value: *mut Il2CppString);
pub extern "C" fn set_text_hook(this: *mut Il2CppObject, value: *mut Il2CppString) {
    if value.is_null() {
        return get_orig_fn!(set_text_hook, SetTextFn)(this, value);
    }

    let config = crate::core::Hachimi::instance().config.load();
    if !config.auto_translate_localize && !config.auto_translate_stories {
        return get_orig_fn!(set_text_hook, SetTextFn)(this, value);
    }

    let orig_str = unsafe { (*value).as_utf16str().to_string() };

    if let Some(trans) = SugoiClient::instance().get_cached(&orig_str) {
        return get_orig_fn!(set_text_hook, SetTextFn)(this, trans.to_il2cpp_string());
    }

    let active_id = crate::core::sugoi_client::ACTIVE_STORY_ID.load(std::sync::atomic::Ordering::Relaxed);
    if let Some(story_pending) = crate::core::sugoi_client::PENDING_STORY_TRANSLATIONS.lock().unwrap().get(&active_id) {
        if let Some(Some(trans)) = story_pending.get(&orig_str) {
            return get_orig_fn!(set_text_hook, SetTextFn)(this, trans.to_il2cpp_string());
        }
    }

    let gen = crate::core::sugoi_client::COMPONENT_GENERATION.load(std::sync::atomic::Ordering::Relaxed);
    ACTIVE_TEXT_MESH_COMPONENTS.lock().unwrap().insert(this as usize, (gen, orig_str));

    get_orig_fn!(set_text_hook, SetTextFn)(this, value);
}

#[cfg(target_os = "windows")]
fn is_object_alive_safe(obj: *mut Il2CppObject) -> bool {
    if obj.is_null() {
        return false;
    }

    microseh::try_seh(|| Object::op_Implicit(obj)).unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn is_object_alive_safe(obj: *mut Il2CppObject) -> bool {
    if obj.is_null() {
        return false;
    }
    unsafe { Object::op_Implicit(obj) }
}

pub fn apply_translations(completed: &[(&String, &String)]) {
    let tracker = ACTIVE_TEXT_MESH_COMPONENTS.lock().unwrap();
    let gen = crate::core::sugoi_client::COMPONENT_GENERATION.load(std::sync::atomic::Ordering::Relaxed);

    #[cfg(target_os = "windows")]
    {
        microseh::try_seh(|| {
            for (&ptr, (entry_gen, saved_orig)) in tracker.iter() {
                if *entry_gen != gen {
                    continue;
                }
                let obj = ptr as *mut Il2CppObject;
                if !is_object_alive_safe(obj) {
                    continue;
                }
                let current_text = get_text(obj);
                let still_matches = !current_text.is_null() && unsafe { (*current_text).as_utf16str().to_string() } == *saved_orig;
                if still_matches {
                    if let Some((_orig, trans)) = completed.iter().find(|(o, _)| **o == *saved_orig) {
                        let unity_string = trans.to_il2cpp_string();
                        get_orig_fn!(set_text_hook, SetTextFn)(obj, unity_string);
                    }
                }
            }
        }).ok();
    }

    #[cfg(not(target_os = "windows"))]
    {
        for (&ptr, (entry_gen, saved_orig)) in tracker.iter() {
            if *entry_gen != gen {
                continue;
            }
            let obj = ptr as *mut Il2CppObject;
            if !is_object_alive_safe(obj) {
                continue;
            }
            let current_text = get_text(obj);
            let still_matches = !current_text.is_null() && unsafe { (*current_text).as_utf16str().to_string() } == *saved_orig;
            if still_matches {
                if let Some((_orig, trans)) = completed.iter().find(|(o, _)| **o == *saved_orig) {
                    let unity_string = trans.to_il2cpp_string();
                    get_orig_fn!(set_text_hook, SetTextFn)(obj, unity_string);
                }
            }
        }
    }
}

pub fn cleanup_components() {
    ACTIVE_TEXT_MESH_COMPONENTS.lock().unwrap().clear();
}

pub fn init(UnityEngine_TextRenderingModule: *const Il2CppImage) {
    get_class_or_return!(UnityEngine_TextRenderingModule, UnityEngine, TextMesh);

    let set_text_addr = get_method_addr(TextMesh, c"set_text", 1);
    new_hook!(set_text_addr, set_text_hook);

    unsafe {
        GET_TEXT_ADDR = get_method_addr(TextMesh, c"get_text", 0);
    }
}
