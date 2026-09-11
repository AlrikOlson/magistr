//! The bootstrap program and the nonce-framed sentinel protocol, retained
//! verbatim from the pinned script tool: the guest's workspace shim serves
//! host call k from an injected response journal when k is already
//! answered, otherwise emits a request sentinel and unwinds; the host services
//! it and re-runs. `scriptArgs = [journalJson, nonce]`, blanked before user
//! code runs.
use serde_json::{Map, Value};

/// Wrap the user source as a function body inside the shim.
pub(super) fn build_guest_program(source: &str) -> String {
    let mut program = String::with_capacity(source.len() + 1600);
    program.push_str(concat!(
        "\"use strict\";(function(){",
        "var __args=scriptArgs;var __journal=JSON.parse(__args[0]);var __nonce=__args[1];",
        "globalThis.scriptArgs=[];",
        "var __calls=0;var __edits=[];var __files=[];",
        "function __emit(p){print(\"\\n@@WB[\"+__nonce+\"]\"+JSON.stringify(p)+\"[\"+__nonce+\"]WB@@\");}",
        "var __PENDING={};",
        "function __host(method,args){",
        "var req={method:method,args:args===undefined?null:args};var i=__calls++;",
        "if(i<__journal.length){var e=__journal[i];",
        "if(JSON.stringify(req)!==e.k){throw {__wbNondet:i};}return e.r;}",
        "__emit({k:\"call\",i:i,req:req});throw __PENDING;}",
        "var workspace={",
        "search:function(a){return __host(\"search\",a);},",
        "read:function(a){return __host(\"read\",a);},",
        "glob:function(p){return __host(\"glob\",p);},",
        "edit:{queue:function(op){__edits.push(op);}},",
        "files:{queue:function(op){__files.push(op);}}};",
        "var __value;var __threw=null;",
        "try{__value=(function(workspace){\n",
    ));
    program.push_str(source);
    program.push_str(concat!(
        "\n})(workspace);}",
        "catch(e){if(e===__PENDING){return;}",
        "if(e&&typeof e.__wbNondet===\"number\"){__emit({k:\"nondet\",i:e.__wbNondet});return;}",
        "__threw=String(e&&e.message!==undefined?e.message:e);}",
        "if(__threw!==null){__emit({k:\"done\",threw:__threw,edits:__edits,files:__files});return;}",
        "var __v;try{__v=JSON.parse(JSON.stringify(__value===undefined?null:__value));}",
        "catch(e2){__emit({k:\"done\",threw:\"return value is not JSON-serializable: \"+String(e2),",
        "edits:__edits,files:__files});return;}",
        "__emit({k:\"done\",value:__v,edits:__edits,files:__files});",
        "})();",
    ));
    program
}

fn frame_markers(nonce: &str) -> (String, String) {
    (format!("@@WB[{nonce}]"), format!("[{nonce}]WB@@"))
}

/// The last nonce-framed sentinel in an exit-0 round's stdout, else `None`.
pub(super) fn last_sentinel(stdout: &str, nonce: &str) -> Option<Map<String, Value>> {
    let (open, close) = frame_markers(nonce);
    let start = stdout.rfind(&open)?;
    let body_start = start + open.len();
    let end = stdout[body_start..].find(&close)? + body_start;
    match serde_json::from_str(&stdout[body_start..end]).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// Strip every sentinel frame from the guest's reported stdout.
pub(super) fn strip_sentinels(stdout: &str, nonce: &str) -> String {
    let (open, close) = frame_markers(nonce);
    let mut out = stdout.to_owned();
    loop {
        let Some(start) = out.find(&open) else {
            return out;
        };
        let Some(end) = out[start + open.len()..].find(&close) else {
            out.truncate(start);
            return out;
        };
        let end = start + open.len() + end + close.len();
        out.replace_range(start..end, "");
    }
}

/// 32 hex characters of host entropy; a user-printed forgery lacks it.
pub(super) fn fresh_nonce() -> std::io::Result<String> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(std::io::Error::other)?;
    Ok(format!("{:032x}", u128::from_be_bytes(random)))
}
