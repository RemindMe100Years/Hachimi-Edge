use crate::il2cpp::{symbols::{get_method_addr}, types::*};

static mut GET_CURRENT_ADDR: usize = 0;
impl_addr_wrapper_fn!(get_current, GET_CURRENT_ADDR, *mut Il2CppObject,);

static mut GET_CURRENTSELECTEDGAMEOBJECT_ADDR: usize = 0;
impl_addr_wrapper_fn!(get_currentSelectedGameObject, GET_CURRENTSELECTEDGAMEOBJECT_ADDR, *mut Il2CppObject, this: *mut Il2CppObject);

type UpdateFn = extern "C" fn(this: *mut Il2CppObject);
extern "C" fn Update(this: *mut Il2CppObject) {
    get_orig_fn!(Update, UpdateFn)(this);

    let mut completed = Vec::new();
    {
        let rx = crate::core::sugoi_client::TRANSLATION_QUEUE.1.lock().unwrap();
        while let Ok(msg) = rx.try_recv() {
            completed.push(msg);
        }
    }

    if completed.is_empty() {
        #[cfg(target_os = "windows")]
        if microseh::try_seh(|| crate::windows::smtc::on_update()).is_err() {
            error!("[smtc] SEH exception in on_update!");
        }
        return;
    }

    {
        let mut cache = crate::core::sugoi_client::TRANSLATION_CACHE.lock().unwrap();
        for (_story_id, orig, trans) in &completed {
            cache.insert(orig.clone(), trans.clone());
        }
    }

    let active_story_id = crate::core::sugoi_client::ACTIVE_STORY_ID.load(std::sync::atomic::Ordering::Relaxed);
    let mut applicable: Vec<(&String, &String)> = Vec::new();
    for (story_id, orig, trans) in &completed {
        if *story_id == 0 || *story_id == active_story_id {
            applicable.push((orig, trans));
        }
    }

    if applicable.is_empty() {
        #[cfg(target_os = "windows")]
        if microseh::try_seh(|| crate::windows::smtc::on_update()).is_err() {
            error!("[smtc] SEH exception in on_update!");
        }
        return;
    }

    crate::il2cpp::hook::UnityEngine_UI::Text::apply_translations(&applicable);
    crate::il2cpp::hook::UnityEngine_TextRenderingModule::TextMesh::apply_translations(&applicable);

    crate::il2cpp::hook::umamusume::StoryTimelineData::apply_pending_clip_updates();

    #[cfg(target_os = "windows")]
    if microseh::try_seh(|| crate::windows::smtc::on_update()).is_err() {
        error!("[smtc] SEH exception in on_update!");
    }
}

pub fn init(UnityEngine_UI: *const Il2CppImage) {
    get_class_or_return!(UnityEngine_UI, "UnityEngine.EventSystems", EventSystem);

    let Update_addr = get_method_addr(EventSystem, c"Update", 0);
    new_hook!(Update_addr, Update);

    unsafe {
        GET_CURRENT_ADDR = get_method_addr(EventSystem, c"get_current", 0);
        GET_CURRENTSELECTEDGAMEOBJECT_ADDR = get_method_addr(EventSystem, c"get_currentSelectedGameObject", 0);
    }
}