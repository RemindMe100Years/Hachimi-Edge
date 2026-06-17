use std::sync::{Arc, Mutex, mpsc::{self, Receiver, Sender}};
use std::sync::atomic::{AtomicU32, AtomicBool};

use fnv::{FnvHashMap, FnvHashSet};
use once_cell::sync::Lazy;
use serde::Serialize;

use super::{Error, Hachimi};

pub struct SugoiClient {
    url: String,
}

static INSTANCE: Lazy<Arc<SugoiClient>> = Lazy::new(|| {
    Arc::new(SugoiClient {
        url: Hachimi::instance().config.load().sugoi_url.as_ref()
            .map(|s| s.clone())
            .unwrap_or_else(|| "http://127.0.0.1:14366".to_owned()),
    })
});

pub static TRANSLATION_QUEUE: Lazy<(Sender<(u32, String, String)>, Mutex<Receiver<(u32, String, String)>>)> = Lazy::new(|| {
    let (tx, rx) = mpsc::channel();
    (tx, Mutex::new(rx))
});

pub static TRANSLATION_CACHE: Lazy<Mutex<FnvHashMap<String, String>>> = Lazy::new(|| {
    Mutex::new(FnvHashMap::default())
});

pub static PENDING_TRANSLATIONS: Lazy<Mutex<FnvHashSet<String>>> = Lazy::new(|| {
    Mutex::new(FnvHashSet::default())
});

pub static ACTIVE_STORY_ID: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));
pub static NEXT_STORY_ID: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(1));
pub static STORY_TL_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
pub static SHUTDOWN: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

pub static PENDING_STORY_TRANSLATIONS: Lazy<Mutex<FnvHashMap<u32, FnvHashMap<String, Option<String>>>>> = Lazy::new(|| {
    Mutex::new(FnvHashMap::default())
});

pub static COMPONENT_GENERATION: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(1));

pub static REQUEST_QUEUE: Lazy<Sender<String>> = Lazy::new(|| {
    let (tx, rx) = mpsc::channel::<String>();
    let translation_tx = TRANSLATION_QUEUE.0.clone();

    std::thread::Builder::new()
        .name("sugoi_worker".into())
        .spawn(move || {
            while let Ok(original) = rx.recv() {
                let mut batch = vec![original];

                while batch.len() < 50 {
                    if let Ok(next) = rx.try_recv() {
                        batch.push(next);
                    } else {
                        break;
                    }
                }

                let client = SugoiClient::instance();

                match client.translate(&batch) {
                    Ok(translated) => {
                        let mut pending = PENDING_TRANSLATIONS.lock().unwrap_or_else(|e| e.into_inner());
                        for (orig, trans) in batch.into_iter().zip(translated.into_iter()) {
                            let _ = translation_tx.send((0, orig.clone(), trans));
                            pending.remove(&orig);
                        }
                    }
                    Err(_) => {
                        let mut pending = PENDING_TRANSLATIONS.lock().unwrap_or_else(|e| e.into_inner());
                        for orig in batch {
                            pending.remove(&orig);
                        }
                    }
                }
            }
        })
        .expect("Failed to spawn sugoi_worker thread");

    tx
});

impl SugoiClient {
    pub fn instance() -> Arc<Self> {
        INSTANCE.clone()
    }

    pub fn get_cached(&self, original: &str) -> Option<String> {
        TRANSLATION_CACHE.lock().unwrap_or_else(|e| e.into_inner()).get(original).cloned()
    }

    pub fn translate_async(&self, original: String) {
        if self.get_cached(&original).is_some() {
            return;
        }

        let mut pending = PENDING_TRANSLATIONS.lock().unwrap_or_else(|e| e.into_inner());
        if pending.insert(original.clone()) {
            let _ = REQUEST_QUEUE.send(original);
        }
    }

    pub fn translate(&self, content: &[String]) -> Result<Vec<String>, Error> {
        let agent = ureq::Agent::new_with_defaults();

        let res = agent.post(&self.url)
            .header("Content-Type", "application/json")
            .header("Connection", "close")
            .send_json(Message::TranslateSentences { content })?;

        let body_str = res.into_body().read_to_string()?;
        Ok(serde_json::from_str(&body_str)?)
    }

    pub fn translate_one(&self, content: String) -> Result<String, Error> {
        let mut res = self.translate(&[content])?;
        if res.len() != 1 {
            return Err(Error::RuntimeError("Server returned invalid amount of translated content".to_owned()));
        }
        Ok(res.pop().unwrap())
    }
}

#[derive(Serialize)]
#[serde(tag = "message")]
enum Message<'a> {
    #[serde(rename = "translate sentences")]
    TranslateSentences {
        content: &'a [String]
    }
}