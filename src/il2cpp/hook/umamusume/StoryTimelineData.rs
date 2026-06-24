use std::ptr::null_mut;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use widestring::Utf16Str;

use once_cell::sync::Lazy;

use crate::{
    core::{ext::Utf16StringExt, utils, Hachimi, SugoiClient},
    il2cpp::{
        ext::{Il2CppStringExt, StringExt}, hook::{umamusume::{StoryTimelineCharaTrackData, StoryTimelineClipData}, UnityEngine_AssetBundleModule::AssetBundle::ASSET_PATH_PREFIX}, symbols::{get_field_from_name, get_field_object_value, get_field_value, set_field_object_value, set_field_value, IList, GCHandle}, types::*
    }
};

use super::{StoryTimelineBlockData, StoryTimelineTextClipData, StoryTimelineTrackData};

struct PendingClipUpdate {
    story_handle: GCHandle,
    story_id: u32,
    dict: StoryTimelineDataDict,
    wp: WrapParams,
    no_wrap: bool,
}

static PENDING_CLIP_UPDATES: Lazy<Mutex<Vec<PendingClipUpdate>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub fn apply_pending_clip_updates() {
    let updates = {
        let mut queue = PENDING_CLIP_UPDATES.lock().unwrap();
        std::mem::take(&mut *queue)
    };

    for update in updates {
        let story_obj = update.story_handle.target();
        if story_obj.is_null() {
            continue;
        }

        if update.story_id != crate::core::sugoi_client::ACTIVE_STORY_ID.load(std::sync::atomic::Ordering::Relaxed) {
            continue;
        }

        if let Some(block_list) = IList::new(get_BlockList(story_obj)) {
            if let Some(title) = &update.dict.title {
                let wrapped_title = apply_wrapping(title, &update.wp, update.no_wrap);
                set_Title(story_obj, wrapped_title.to_il2cpp_string());
            }

            for (i, block_data) in block_list.iter().enumerate().skip(1) {
                let Some(clip_data) = StoryTimelineBlockData::get_text_clip(block_data) else {
                    continue;
                };
                let adjusted_i = i - 1;
                let Some(block) = update.dict.text_block_list.get(adjusted_i) else {
                    continue;
                };

                if let Some(name) = &block.name {
                    let wrapped_name = apply_wrapping(name, &update.wp, update.no_wrap);
                    StoryTimelineTextClipData::set_Name(clip_data, wrapped_name.to_il2cpp_string());
                }

                if let Some(text) = &block.text {
                    let wrapped_text = apply_wrapping(text, &update.wp, update.no_wrap);
                    StoryTimelineTextClipData::set_Text(clip_data, wrapped_text.to_il2cpp_string());
                }

                if let Some(choice_list) = IList::new(StoryTimelineTextClipData::get_ChoiceDataList(clip_data)) {
                    for (j, choice_data) in choice_list.iter().enumerate() {
                        if let Some(ct) = block.choice_data_list.get(j) {
                            if !ct.is_empty() {
                                let wrapped_choice = apply_wrapping(ct, &update.wp, update.no_wrap);
                                StoryTimelineTextClipData::ChoiceData::set_Text(choice_data, wrapped_choice.to_il2cpp_string());
                            }
                        }
                    }
                }

                if let Some(color_list) = IList::new(StoryTimelineTextClipData::get_ColorTextInfoList(clip_data)) {
                    for (j, color_info) in color_list.iter().enumerate() {
                        if let Some(ct) = block.color_text_info_list.get(j) {
                            if !ct.is_empty() {
                                let wrapped_color = apply_wrapping(ct, &update.wp, update.no_wrap);
                                StoryTimelineTextClipData::ColorTextInfo::set_Text(color_info, wrapped_color.to_il2cpp_string());
                            }
                        }
                    }
                }
            }
        }
    }
}

const CLIP_TEXT_LINE_WIDTH: i32 = 21;
const CLIP_TEXT_LINE_COUNT: i32 = 3;
const CLIP_TEXT_FONT_SIZE_DEFAULT: i32 = 42;
/*
const CLIP_TEXT_FONT_SIZE_LARGE: i32 = 84;
const CLIP_TEXT_FONT_SIZE_SMALL: i32 = 32;
const CLIP_TEXT_FONT_SIZE_BOLD_CAPTION: i32 = 64;
*/

// probably?
const STORY_VIEW_CLIP_TEXT_LINE_WIDTH: i32 = 32;

static mut CLASS: *mut Il2CppClass = null_mut();
pub fn class() -> *mut Il2CppClass {
    unsafe { CLASS }
}

static mut TITLE_FIELD: *mut FieldInfo = null_mut();
fn set_Title(this: *mut Il2CppObject, value: *mut Il2CppString) {
    set_field_object_value(this, unsafe { TITLE_FIELD }, value);
}

fn get_Title(this: *mut Il2CppObject) -> *mut Il2CppString {
    get_field_object_value(this, unsafe { TITLE_FIELD })
}

// List<StoryTimelineBlockData>
static mut BLOCKLIST_FIELD: *mut FieldInfo = null_mut();
pub fn get_BlockList(this: *mut Il2CppObject) -> *mut Il2CppObject {
    get_field_object_value(this, unsafe { BLOCKLIST_FIELD })
}

static mut TYPEWRITECOUNTPERSECOND_FIELD: *mut FieldInfo = null_mut();
fn get_TypewriteCountPerSecond(this: *mut Il2CppObject) -> i32 {
    get_field_value(this, unsafe { TYPEWRITECOUNTPERSECOND_FIELD })
}

fn set_TypewriteCountPerSecond(this: *mut Il2CppObject, value: i32) {
    set_field_value(this, unsafe { TYPEWRITECOUNTPERSECOND_FIELD }, &value);
}

static mut LENGTH_FIELD: *mut FieldInfo = null_mut();
fn set_Length(this: *mut Il2CppObject, value: i32) {
    set_field_value(this, unsafe { LENGTH_FIELD }, &value);
}

// (Aliases are there for tlg compatibility)
#[derive(Serialize, Deserialize, Default)]
struct StoryTimelineDataDict {
    #[serde(alias = "Title")]
    title: Option<String>,

    #[serde(alias = "TextBlockList")]
    #[serde(default)]
    text_block_list: Vec<TextBlockDict>,

    #[serde(default)]
    no_wrap: bool
}

#[derive(Serialize, Deserialize, Default)]
struct TextBlockDict {
    #[serde(alias = "Name")]
    name: Option<String>,

    #[serde(alias = "Text")]
    text: Option<String>,

    #[serde(alias = "ChoiceDataList")]
    #[serde(default)]
    choice_data_list: Vec<String>,

    #[serde(alias = "ColorTextInfoList")]
    #[serde(default)]
    color_text_info_list: Vec<String>,

    new_clip_length: Option<i32>
}

// hook::UnityEngine_AssetBundleModule::AssetBundle
// name:
// - assets/_gallopresources/bundle/resources/home/data/xxxxx/yy/hometimeline_xxxxx_yy_zzzzzzz.asset
// - assets/_gallopresources/bundle/resources/story/data/xx/yyyy/storytimeline_xxyyyyzzz.asset
pub fn on_LoadAsset(_bundle: *mut Il2CppObject, this: *mut Il2CppObject, name: &Utf16Str) {
    if !name.starts_with(ASSET_PATH_PREFIX) {
        // ???
        return;
    }

    let hachimi = Hachimi::instance();
    let mut tcps = get_TypewriteCountPerSecond(this) as f32;
    let tcps_mult = hachimi.config.load().story_tcps_multiplier;
    if tcps_mult != 1.0 {
        tcps = (tcps * tcps_mult).round();
        set_TypewriteCountPerSecond(this, tcps as i32);
    }

    let base_path = name[ASSET_PATH_PREFIX.len()..].path_basename();
    let dict_path = base_path.to_string() + ".json";

    let localized_data = hachimi.localized_data.load();

    let is_story_view = base_path.starts_with("story/data/") && (
        base_path[11..].starts_with("02/") ||
        base_path[11..].starts_with("04/") ||
        base_path[11..].starts_with("09/")
    );

    // Init wrapping parameters
    let mut line_count = CLIP_TEXT_LINE_COUNT;
    if let Some(offset) = localized_data.config.story_line_count_offset {
        line_count += offset;
    }

    let mut font_size = CLIP_TEXT_FONT_SIZE_DEFAULT;
    let mut line_width = CLIP_TEXT_LINE_WIDTH;
    let mut story_view_line_width = STORY_VIEW_CLIP_TEXT_LINE_WIDTH;
    if let Some(mult) = localized_data.config.text_frame_font_size_multiplier {
        font_size = (font_size as f32 * mult).round() as i32;
        line_width = (line_width as f32 / mult).round() as i32;
        story_view_line_width = (story_view_line_width as f32 / mult).round() as i32;
    }

    let wp = WrapParams {
        is_story_view,
        line_count,
        font_size,
        line_width,
        story_view_line_width,
        size: StoryTimelineTextClipData::get_Size(this),
    };

    let Some(dict): Option<StoryTimelineDataDict> = localized_data.load_assets_dict(Some(&dict_path)).or_else(|| {
        if hachimi.config.load().auto_translate_stories {
            let Some(full_dict_path) = localized_data.get_assets_path(&dict_path) else {
                return None;
            };

            // check if file exists
            if std::fs::metadata(&full_dict_path).is_ok() {
                return None;
            }

            let guard = crate::core::sugoi_client::STORY_TL_LOCK.try_lock();
            if guard.is_err() {
                return None;
            }

            let story_id = crate::core::sugoi_client::NEXT_STORY_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            crate::core::sugoi_client::ACTIVE_STORY_ID.store(story_id, std::sync::atomic::Ordering::Relaxed);

            {
                let mut pending = crate::core::sugoi_client::PENDING_STORY_TRANSLATIONS.lock().unwrap();
                pending.retain(|id, _| *id == story_id);
            }

            crate::core::sugoi_client::COMPONENT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            crate::il2cpp::hook::UnityEngine_UI::Text::cleanup_components();
            crate::il2cpp::hook::UnityEngine_TextRenderingModule::TextMesh::cleanup_components();

            let story_handle = GCHandle::new(this, false);
            dispatch_auto_tl_async(this, full_dict_path, wp.clone(), story_id, guard.unwrap(), story_handle);
            None
        }
        else {
            None
        }
    }) else {
        // Clip length adjustment independent of story patching
        // No need to adjust length if speed is faster
        if tcps_mult < 1.0 {
            adjust_clips_length_with_tcps(this, tcps);
        }
        return;
    };
    debug!("{}", dict_path);

    if let Some(title) = &dict.title {
        set_Title(this, title.to_il2cpp_string());
    }

    let Some(block_list) = IList::new(get_BlockList(this)) else {
        return;
    };

    let mut total_len = 0;
    let mut total_len_changed = false;
    for (mut i, block_data) in block_list.iter().enumerate() {
        let orig_block_len = StoryTimelineBlockData::get_BlockLength(block_data);
        total_len += orig_block_len;

        // First block is always empty, skip over it
        if i == 0 { continue; }
        i -= 1;

        let Some(text_block_dict) = dict.text_block_list.get(i) else {
            warn!("text block {} not found in dict: {}", i, dict_path);
            break;
        };

        let Some(clip_data) = StoryTimelineBlockData::get_text_clip(block_data) else {
            continue;
        };

        if let Some(name) = &text_block_dict.name {
            StoryTimelineTextClipData::set_Name(clip_data, name.to_il2cpp_string());
        }

        if let Some(text) = &text_block_dict.text {
            let mut modified_text = None;
            if !dict.no_wrap {
                if is_story_view {
                    // Sizing tags are not used at all in main stories, simply wrap it
                    // Add an extra space to each line because the vertical log screen ignores newlines
                    if let Some(wrapped) = utils::wrap_text(text, story_view_line_width) {
                        modified_text = Some(wrapped.join(" \n"));
                    }
                }
                else {
                    let size = StoryTimelineTextClipData::get_Size(this);
                    if size == StoryTimelineTextClipData::FontSize_Default {
                        if let Some(fitted) = utils::wrap_fit_text(text, line_width, line_count, font_size) {
                            modified_text = Some(fitted);
                        }
                    }
                    // not doing anything with text of other sizes for now...
                }
            }
            let new_text = modified_text.as_ref().unwrap_or(text);
            StoryTimelineTextClipData::set_Text(clip_data, new_text.to_il2cpp_string());

            // Adjust clip length
            if localized_data.config.auto_adjust_story_clip_length ||
                text_block_dict.new_clip_length.is_some() ||
                tcps_mult < 1.0
            {
                let new_clip_len = text_block_dict.new_clip_length.unwrap_or_else(|| {
                    let text_len = utils::IsolateTags::new(new_text).fold(0, |total_len, (s, is_not_tag)| 
                        if is_not_tag { total_len + s.chars().count() } else { total_len }
                    );
                    // Everything else down here is in the unit of frames at 30fps
                    let typewrite_len = get_typewrite_length(text_len, tcps);
                    return StoryTimelineTextClipData::get_WaitFrame(clip_data) +
                        typewrite_len.max(StoryTimelineTextClipData::get_VoiceLength(clip_data));
                });

                let orig_clip_len = StoryTimelineClipData::get_ClipLength(clip_data);
                if new_clip_len > orig_clip_len {
                    let new_block_len = apply_clip_length(
                        clip_data, orig_clip_len, new_clip_len,
                        block_data, orig_block_len
                    );
                    let block_len_diff = new_block_len - orig_block_len;
                    total_len += block_len_diff;
                    total_len_changed = true;
                }
            }
        }

        // IList::new checks for null, no need to do so explicitly
        let choice_data_list_obj = StoryTimelineTextClipData::get_ChoiceDataList(clip_data);
        if let Some(choice_data_list) = IList::new(choice_data_list_obj) {
            for (j, choice_data) in choice_data_list.iter().enumerate() {
                if let Some(text) = text_block_dict.choice_data_list.get(j) {
                    if !text.is_empty() {
                        StoryTimelineTextClipData::ChoiceData::set_Text(choice_data, text.to_il2cpp_string())
                    }
                }
                else {
                    warn!("choice data {} of block {} not found in dict: {}", j, i, dict_path);
                }
            }
        }

        let color_text_info_list_obj = StoryTimelineTextClipData::get_ColorTextInfoList(clip_data);
        if let Some(color_text_info_list) = IList::new(color_text_info_list_obj) {
            for (j, color_text_info) in color_text_info_list.iter().enumerate() {
                if let Some(text) = text_block_dict.color_text_info_list.get(j) {
                    if !text.is_empty() {
                        StoryTimelineTextClipData::ColorTextInfo::set_Text(color_text_info, text.to_il2cpp_string())
                    }
                }
                else {
                    warn!("color text info {} of block {} not found in dict: {}", j, i, dict_path);
                }
            }
        }
    }

    if total_len_changed {
        set_Length(this, total_len);
    }
}

fn get_typewrite_length(text_len: usize, tcps: f32) -> i32 {
    (text_len as f32 / tcps * 30.0).round() as i32 // len / cps * fps
}

fn adjust_clips_length_with_tcps(this: *mut Il2CppObject, tcps: f32) {
    let Some(block_list) = IList::new(get_BlockList(this)) else {
        return;
    };
    let mut block_list_iter = block_list.iter();

    // First block is always empty, no need to adjust length
    let Some(first_block_data) = block_list_iter.next() else {
        return;
    };
    let mut total_len = StoryTimelineBlockData::get_BlockLength(first_block_data);

    for block_data in block_list_iter {
        let orig_block_len = StoryTimelineBlockData::get_BlockLength(block_data);
        let Some(clip_data) = StoryTimelineBlockData::get_text_clip(block_data) else {
            total_len += orig_block_len;
            continue;
        };
        let text = StoryTimelineTextClipData::get_Text(clip_data);

        total_len += if text.is_null() {
            orig_block_len
        }
        else {
            let orig_clip_len = StoryTimelineClipData::get_ClipLength(clip_data);
            let new_clip_len = get_typewrite_length(unsafe { (*text).as_utf16str().chars().count() }, tcps);

            if new_clip_len > orig_clip_len {
                apply_clip_length(clip_data, orig_clip_len, new_clip_len, block_data, orig_block_len)
            }
            else {
                orig_block_len
            }
        }
    }

    set_Length(this, total_len);
}

/// Returns new block length
fn apply_clip_length(
    clip_data: *mut Il2CppObject, orig_clip_len: i32, new_clip_len: i32,
    block_data: *mut Il2CppObject, orig_block_len: i32
) -> i32 {
    StoryTimelineClipData::set_ClipLength(clip_data, new_clip_len);
    let new_block_len = StoryTimelineClipData::get_StartFrame(clip_data) + new_clip_len + 1;
    StoryTimelineBlockData::set_BlockLength(block_data, new_block_len);

    let clip_len_diff = new_clip_len - orig_clip_len;

    // Adjust anim lengths
    if let Some(chara_track_list) = <IList>::new(StoryTimelineBlockData::get_CharacterTrackList(block_data)) {
        for chara_track_data in chara_track_list.iter() {
            for motion_track_data in StoryTimelineCharaTrackData::motion_track_data_values(chara_track_data) {
                let Some(clip_list) = <IList>::new(StoryTimelineTrackData::get_ClipList(motion_track_data)) else {
                    continue;
                };
                let Some(clip_data) = clip_list.get(clip_list.count() - 1) else {
                    continue;
                };

                let orig_motion_clip_len = StoryTimelineClipData::get_ClipLength(clip_data);
                let new_motion_clip_len = orig_motion_clip_len + clip_len_diff;
                StoryTimelineClipData::set_ClipLength(clip_data, new_motion_clip_len);
            }
        }
    }

    // Adjust screen effect lengths
    if let Some(se_track_list) = <IList>::new(StoryTimelineBlockData::get_ScreenEffectTrackList(block_data)) {
        for se_track_data in se_track_list.iter() {
            let Some(clip_list) = <IList>::new(StoryTimelineTrackData::get_ClipList(se_track_data)) else {
                continue;
            };
            let Some(clip_data) = clip_list.get(clip_list.count() - 1) else {
                continue;
            };

            let start_frame = StoryTimelineClipData::get_StartFrame(clip_data);
            let orig_se_clip_len = StoryTimelineClipData::get_ClipLength(clip_data);
            // if it extends to the end of the block
            if start_frame + orig_se_clip_len < orig_block_len {
                continue;
            }

            let new_se_clip_len = orig_se_clip_len + clip_len_diff;
            StoryTimelineClipData::set_ClipLength(clip_data, new_se_clip_len);
        }
    }

    new_block_len
}

#[derive(Clone)]
struct WrapParams {
    is_story_view: bool,
    line_count: i32,
    font_size: i32,
    line_width: i32,
    story_view_line_width: i32,
    size: i32,
}

fn apply_wrapping(text: &str, wp: &WrapParams, no_wrap: bool) -> String {
    if no_wrap {
        return text.to_string();
    }
    if wp.is_story_view {
        if let Some(wrapped) = utils::wrap_text(text, wp.story_view_line_width) {
            return wrapped.join(" \n");
        }
    } else if wp.size == StoryTimelineTextClipData::FontSize_Default {
        if let Some(fitted) = utils::wrap_fit_text(text, wp.line_width, wp.line_count, wp.font_size) {
            return fitted;
        }
    }
    text.to_string()
}

fn dispatch_auto_tl_async(this: *mut Il2CppObject, full_dict_path: std::path::PathBuf, wp: WrapParams, story_id: u32, _lock_guard: std::sync::MutexGuard<'static, ()>, story_handle: GCHandle) {
    let Some(block_list) = <IList>::new(get_BlockList(this)) else {
        return;
    };

    let mut dict = StoryTimelineDataDict::default();

    let title = get_Title(this);
    if !title.is_null() && unsafe { (*title).length > 0 } {
        let title_str = unsafe { (*title).as_utf16str().to_string() };
        if title_str != "0" {
            dict.title = Some(title_str);
        }
    }

    // first block is empty
    for block_data in block_list.iter().skip(1) {
        let mut block_dict = TextBlockDict::default();

        let Some(clip_data) = StoryTimelineBlockData::get_text_clip(block_data) else {
            dict.text_block_list.push(block_dict);
            continue;
        };

        let name = StoryTimelineTextClipData::get_Name(clip_data);
        if !name.is_null() && unsafe { (*name).length > 0 } {
            let name_str = unsafe { (*name).as_utf16str().to_string() };
            if name_str != "モノローグ" && name_str != "<username>" {
                block_dict.name = Some(name_str);
            }
        }

        let text = StoryTimelineTextClipData::get_Text(clip_data);
        if !text.is_null() && unsafe { (*text).length > 0 } {
            block_dict.text = Some(unsafe { (*text).as_utf16str().to_string() });
        }

        let choice_data_list_obj = StoryTimelineTextClipData::get_ChoiceDataList(clip_data);
        if let Some(choice_data_list) = IList::new(choice_data_list_obj) {
            for choice_data in choice_data_list.iter() {
                let text = StoryTimelineTextClipData::ChoiceData::get_Text(choice_data);
                if !text.is_null() && unsafe { (*text).length > 0 } {
                    block_dict.choice_data_list.push(unsafe { (*text).as_utf16str().to_string() });
                } else {
                    block_dict.choice_data_list.push(String::new());
                }
            }
        }

        let color_text_info_list_obj = StoryTimelineTextClipData::get_ColorTextInfoList(clip_data);
        if let Some(color_text_info_list) = IList::new(color_text_info_list_obj) {
            for color_text_info in color_text_info_list.iter() {
                let text = StoryTimelineTextClipData::ColorTextInfo::get_Text(color_text_info);
                if !text.is_null() && unsafe { (*text).length > 0 } {
                    block_dict.color_text_info_list.push(unsafe { (*text).as_utf16str().to_string() });
                } else {
                    block_dict.color_text_info_list.push(String::new());
                }
            }
        }

        dict.text_block_list.push(block_dict);
    }

    std::thread::spawn(move || {
        let sugoi = SugoiClient::instance();
        let tx = crate::core::sugoi_client::TRANSLATION_QUEUE.0.clone();

        let save_dict = |current_dict: &StoryTimelineDataDict| {
            if let Some(p) = full_dict_path.parent() {
                let _ = std::fs::create_dir_all(p);
                if let Err(e) = crate::core::utils::write_json_file(current_dict, &full_dict_path) {
                    error!("Failed to save incremental TL dict: {}", e);
                }
            }
        };

        let no_wrap = dict.no_wrap;

        let process_wrap = |text: &str, is_text_block: bool| -> String {
            if no_wrap || !is_text_block {
                return text.to_string();
            }
            if wp.is_story_view {
                if let Some(wrapped) = crate::core::utils::wrap_text(text, wp.story_view_line_width) {
                    return wrapped.join(" \n");
                }
            } else {
                if wp.size == StoryTimelineTextClipData::FontSize_Default {
                    if let Some(fitted) = crate::core::utils::wrap_fit_text(text, wp.line_width, wp.line_count, wp.font_size) {
                        return fitted;
                    }
                }
            }
            text.to_string()
        };

        let mut tl_batch: Vec<String> = Vec::new();
        let mut names_tmp: Vec<String> = Vec::new();
        let mut name_indices: fnv::FnvHashMap<String, usize> = fnv::FnvHashMap::default();

        let title = &dict.title;
        if let Some(t) = title {
            if !t.is_empty() {
                tl_batch.push(t.clone());
            }
        }

        for block in &dict.text_block_list {
            if let Some(name) = &block.name {
                if !name.is_empty() {
                    if !name_indices.contains_key(name) {
                        name_indices.insert(name.clone(), names_tmp.len());
                        names_tmp.push(name.clone());
                    }
                }
            }

            if let Some(text) = &block.text {
                tl_batch.push(text.clone());
            }

            for choice in &block.choice_data_list {
                tl_batch.push(choice.clone());
            }

            for color_text in &block.color_text_info_list {
                tl_batch.push(color_text.clone());
            }
        }

        let names_count = names_tmp.len();
        tl_batch.append(&mut names_tmp);

        {
            let mut pending = crate::core::sugoi_client::PENDING_STORY_TRANSLATIONS.lock().unwrap();
            let story_pending = pending.entry(story_id).or_insert_with(fnv::FnvHashMap::default);
            for text in &tl_batch {
                if !text.is_empty() {
                    story_pending.entry(text.clone()).or_insert(None);
                }
            }
        }

        let translations = if tl_batch.is_empty() {
            Vec::new()
        } else {
            match sugoi.translate(&tl_batch) {
                Ok(t) => t,
                Err(e) => {
                    error!("Batch translation failed: {}", e);
                    return;
                }
            }
        };

        let mut translated = translations;
        let translated_names = if names_count > 0 {
            translated.split_off(translated.len() - names_count)
        } else {
            Vec::new()
        };

        let mut name_trans_map: fnv::FnvHashMap<String, String> = fnv::FnvHashMap::default();
        for (idx, trans) in translated_names.iter().enumerate() {
            for (name, &i) in name_indices.iter() {
                if i == idx {
                    name_trans_map.insert(name.clone(), trans.clone());
                    break;
                }
            }
        }

        let mut tl_iter = translated.into_iter();
        let mut updates_made = 0;

        if let Some(title) = &mut dict.title {
            if !title.is_empty() {
                if let Some(t) = tl_iter.next() {
                    {
                        let mut pending = crate::core::sugoi_client::PENDING_STORY_TRANSLATIONS.lock().unwrap();
                        if let Some(story_pending) = pending.get_mut(&story_id) {
                            story_pending.insert(title.clone(), Some(t.clone()));
                        }
                    }
                    let _ = tx.send((story_id, title.clone(), t.clone()));
                    *title = t;
                    updates_made += 1;
                }
            }
        }

        for i in 0..dict.text_block_list.len() {
            let block = &mut dict.text_block_list[i];

            if let Some(name) = &block.name {
                if let Some(translated_name) = name_trans_map.get(name) {
                    {
                        let mut pending = crate::core::sugoi_client::PENDING_STORY_TRANSLATIONS.lock().unwrap();
                        if let Some(story_pending) = pending.get_mut(&story_id) {
                            story_pending.insert(name.clone(), Some(translated_name.clone()));
                        }
                    }
                    let _ = tx.send((story_id, name.clone(), translated_name.clone()));
                    block.name = Some(translated_name.clone());
                    updates_made += 1;
                }
            }

            if let Some(text) = &mut block.text {
                if let Some(t) = tl_iter.next() {
                    let final_t = process_wrap(&t, true);
                    {
                        let mut pending = crate::core::sugoi_client::PENDING_STORY_TRANSLATIONS.lock().unwrap();
                        if let Some(story_pending) = pending.get_mut(&story_id) {
                            story_pending.insert(text.clone(), Some(final_t.clone()));
                        }
                    }
                    let _ = tx.send((story_id, text.clone(), final_t));
                    *text = t;
                    updates_made += 1;
                }
            }

            for choice in block.choice_data_list.iter_mut() {
                if let Some(t) = tl_iter.next() {
                    let final_t = process_wrap(&t, true);
                    {
                        let mut pending = crate::core::sugoi_client::PENDING_STORY_TRANSLATIONS.lock().unwrap();
                        if let Some(story_pending) = pending.get_mut(&story_id) {
                            story_pending.insert(choice.clone(), Some(final_t.clone()));
                        }
                    }
                    let _ = tx.send((story_id, choice.clone(), final_t));
                    *choice = t;
                    updates_made += 1;
                }
            }

            for color_text in block.color_text_info_list.iter_mut() {
                if let Some(t) = tl_iter.next() {
                    let final_t = process_wrap(&t, false);
                    {
                        let mut pending = crate::core::sugoi_client::PENDING_STORY_TRANSLATIONS.lock().unwrap();
                        if let Some(story_pending) = pending.get_mut(&story_id) {
                            story_pending.insert(color_text.clone(), Some(final_t.clone()));
                        }
                    }
                    let _ = tx.send((story_id, color_text.clone(), final_t));
                    *color_text = t;
                    updates_made += 1;
                }
            }

            if updates_made > 0 {
                save_dict(&dict);
                updates_made = 0;
            }
        }

        let no_wrap = dict.no_wrap;
        {
            let mut queue = PENDING_CLIP_UPDATES.lock().unwrap();
            queue.push(PendingClipUpdate {
                story_handle,
                story_id,
                dict,
                wp,
                no_wrap,
            });
        }
        if !crate::core::sugoi_client::SHUTDOWN.load(std::sync::atomic::Ordering::SeqCst) {
            // Clip updates will be applied by EventSystem::Update on main thread
        }
    });
}

pub fn init(umamusume: *const Il2CppImage) {
    get_class_or_return!(umamusume, Gallop, StoryTimelineData);

    unsafe {
        CLASS = StoryTimelineData;
        TITLE_FIELD = get_field_from_name(StoryTimelineData, c"Title");
        BLOCKLIST_FIELD = get_field_from_name(StoryTimelineData, c"BlockList");
        TYPEWRITECOUNTPERSECOND_FIELD = get_field_from_name(StoryTimelineData, c"TypewriteCountPerSecond");
        LENGTH_FIELD = get_field_from_name(StoryTimelineData, c"Length");
    }
}