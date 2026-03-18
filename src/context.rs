// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Conversation persistence. conv:default:messages in sled.

use crate::storage;
use serde::{Deserialize, Serialize};

const KEY: &[u8] = b"conv:default:messages";
const CAP: usize = 20;

/// t91=Message. Chat message with role and content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub struct t91 {
    pub role: String,
    pub content: String,
}

/// f73 = store_message. Append role+content, cap at CAP.
pub fn f73(store: &storage::t12, role: &str, content: &str) -> Result<(), storage::E0> {
    let mut msgs = f74(store).unwrap_or_default();
    msgs.push(t91 {
        role: role.into(),
        content: content.into(),
    });
    if msgs.len() > CAP {
        msgs.drain(0..(msgs.len() - CAP));
    }
    store.f40(KEY, &msgs)
}

/// f74 = load_messages. Load conversation for chat.
pub fn f74(store: &storage::t12) -> Result<Vec<t91>, storage::E0> {
    Ok(store.f41(KEY)?.unwrap_or_default())
}
