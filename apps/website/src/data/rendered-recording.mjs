import {createHash} from "node:crypto";
import recording from "../../public/demo/tic-tac-toe/recording.json" with {type:"json"};
import {renderRecording} from "../../scripts/highlight-recording.mjs";
export const rendered = await renderRecording(recording);
export const playback = JSON.stringify({version:1,events:rendered.map(({id,terminal,diff})=>({id,terminal,changes:diff?{added:diff.added,removed:diff.removed,first:diff.first}:null}))});
export const playbackUrl = "/demo/tic-tac-toe/playback.json?v="+createHash("sha256").update(playback).digest("hex").slice(0,12);
