import {matchesText} from "./work_search.mjs";
export function filterFeedback(records,{query,kind,status}){
  return records.filter(record=>(kind==="all"||record.kind===kind)&&(status==="all"||record.status===status)&&matchesText([
    record.body,record.from,record.content?.summary,...(record.content?.facts??[]).flatMap(f=>[f.label,f.value]),
    ...(record.content?.sections??[]).flatMap(s=>[s.heading,s.prose,...(s.list??[]).map(e=>e.text)]),
    ...record.enrichment.flatMap(note=>[note.summary,...note.sources])
  ].join("\n"),query));
}
