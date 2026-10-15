import {element} from "./work_dom.mjs";
import {prose} from "./work_prose.mjs";
import {allowedDestination} from "./transcript.mjs";
export function feedbackResearch(record,{copy,openLink}){
  const section=element("section","","feedback-research");section.setAttribute("aria-label","Feedback research");section.append(element("h4","Research"));
  if(!record.enrichment.length){section.append(element("p","No research has been recorded."));return section;}
  for(const [index,note] of record.enrichment.entries()){
    const details=element("details","","work-check"),summary=element("summary"),key=record.id+":research:"+index;
    details.dataset.readingKey=key;summary.dataset.focusKey=key;summary.append(element("strong","Research note "+(index+1)),element("span",new Date(note.at).toLocaleString()));details.append(summary);
    details.append(prose("Research summary",note.summary,key+":summary",copy),element("p","Reported confidence: "+Math.round(note.confidence*100)+"%.","work-record-status"));
    const sources=element("section","","feedback-sources");sources.append(element("h5","Sources"));
    for(const [sourceIndex,source] of note.sources.entries()){
      const row=element("article");row.append(element("p",source,"work-artifact-reference"));
      const copyButton=element("button","Copy reference");copyButton.dataset.focusKey=key+":source:"+sourceIndex;copyButton.onclick=()=>copy(source);row.append(copyButton);
      if(allowedDestination(source)){const open=element("button","Open link");open.onclick=()=>openLink(source);row.append(open);}
      sources.append(row);
    }
    if(!note.sources.length)sources.append(element("p","No sources were recorded."));details.append(sources);section.append(details);
  }
  return section;
}
