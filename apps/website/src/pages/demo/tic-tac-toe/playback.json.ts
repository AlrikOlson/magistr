import {playback} from "../../../data/rendered-recording.mjs";
export function GET(){return new Response(playback,{headers:{"Content-Type":"application/json"}});}
