import {element,inlineFacts} from "./work_dom.mjs";
import {prose} from "./work_prose.mjs";
import {allowedDestination} from "./transcript.mjs";
export function feedbackResearch(record,{copy,openLink,references}){
  const section=element("section","","feedback-research");section.setAttribute("aria-label","Feedback research");section.append(element("h4","Research"));
  if(!record.enrichment.length){section.querySelector("h4").append(element("span","None","work-record-count"));return section;}
  for(const [index,note] of record.enrichment.entries()){
    const details=element("details","","work-check"),summary=element("summary"),key=record.id+":research:"+index;
    details.dataset.readingKey=key;summary.dataset.focusKey=key;summary.append(element("strong","Research note "+(index+1)),element("span",note.sources.length+" "+(note.sources.length===1?"source":"sources")));details.append(summary);
    const date=new Date(note.at),time=element("time",Number.isNaN(date.getTime())?"Not recorded":date.toLocaleString());if(!Number.isNaN(date.getTime()))time.dateTime=date.toISOString();
    const facts=inlineFacts([["Recorded",time],["Reported confidence",note.confidence==null?null:Math.round(note.confidence*100)+"%"]]);facts.classList.add("feedback-research-facts");details.append(facts,prose("Research summary",note.summary,key+":summary",copy));
    if(note.think_step!=null){const related=references(["think:"+note.think_step],"Reasoning behind this note");if(related)details.append(related);}
    const sources=element("section","","feedback-sources");sources.append(element("h5","Sources"));
    for(const [sourceIndex,source] of note.sources.entries()){
      const row=element("article");row.append(element("p",source,"work-artifact-reference"));
      const copyButton=element("button","Copy reference");copyButton.dataset.focusKey=key+":source:"+sourceIndex;copyButton.onclick=()=>copy(source);const controls=element("div","","work-artifact-actions");controls.append(copyButton);row.append(controls);
      if(allowedDestination(source)){const open=element("button","Open link");open.onclick=()=>openLink(source);controls.append(open);}
      sources.append(row);
    }
    if(!note.sources.length)sources.append(element("p","No sources were recorded."));details.append(sources);section.append(details);
  }
  return section;
}
