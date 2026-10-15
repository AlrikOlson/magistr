import {element} from "./work_dom.mjs";
import {pageControls} from "./work_search.mjs";
import {filterFeedback} from "./work_feedback_search.mjs";
import {feedbackResearch} from "./work_feedback_research.mjs";
import {prose} from "./work_prose.mjs";
export const feedbackStatus={new:"New",triaged:"Reviewed",researched:"Researched",surfaced:"Raised for review",promoted:"Added to roadmap",dismissed:"Dismissed"};
export const feedbackKind={question:"Question",idea:"Idea",concern:"Concern",bug:"Bug",feedback:"Feedback"};
export const feedbackTitle=record=>record.content?.summary||(record.body.length>180?record.body.slice(0,180)+"…":record.body);
export function feedbackDetail(record,actions){
  const {copy}=actions;
  const article=element("article","","work-record feedback-detail"),title=element("h3",feedbackTitle(record),"roadmap-track-title");title.tabIndex=-1;title.dataset.focusKey="feedback-title";article.append(title);
  article.append(element("p",feedbackKind[record.kind]+" · "+feedbackStatus[record.status],"work-record-status"),element("p","From "+record.from));
  for(const [label,value] of [["Received",record.created],["Raised for review",record.surfaced_at],["Reminder set for",record.snooze_until]])if(value)article.append(element("p",label+" "+new Date(value).toLocaleString()+".","roadmap-relation-state"));
  article.append(prose("Feedback",record.body,record.id+":body",copy));
  if(record.content){
    const facts=element("dl","","roadmap-content-facts");for(const fact of record.content.facts??[]){const pair=element("div");pair.append(element("dt",fact.label),element("dd",fact.value));facts.append(pair);}if(facts.children.length)article.append(facts);
    for(const [index,section] of (record.content.sections??[]).entries()){
      if(section.prose)article.append(prose(section.heading,section.prose,record.id+":section:"+index,copy));else article.append(element("h4",section.heading));
      const list=element("ul","","roadmap-checklist");for(const entry of section.list??[]){const row=element("li");if(entry.done!=null)row.append(element("span",entry.done?"Completed":"To do","roadmap-checklist-state"));row.append(element("span",entry.text));list.append(row);}article.append(list);
    }
  }
  article.append(feedbackResearch(record,actions));return article;
}
export class FeedbackBrowser{
  constructor(root,changed,copy,openLink){this.root=root;this.changed=changed;this.actions={copy,openLink};this.reset();}
  reset(){this.id=null;this.page=0;this.query="";this.kind="all";this.status="all";}
  open(id){this.originScroll=this.root.scrollTop;this.id=id;this.changed();this.root.scrollTop=0;this.root.querySelector("h3")?.focus({preventScroll:true});}
  back(){const id=this.id;this.id=null;this.changed();this.root.scrollTop=this.originScroll;[...this.root.querySelectorAll("[data-feedback-id]")].find(node=>node.dataset.feedbackId===id)?.focus({preventScroll:true});}
  render(records){
    const article=element("article","","work-record");
    if(this.id){
      const back=element("button","Back to feedback");back.dataset.focusKey="feedback-back";back.onclick=()=>this.back();article.append(back);
      const record=records.find(record=>record.id===this.id);article.append(record?feedbackDetail(record,this.actions):element("h3","This feedback is no longer available."));return article;
    }
    records=filterFeedback(records,this);
    article.append(element("h3","Feedback","roadmap-track-title"));
    const pages=pageControls(records.length,this.page,page=>{this.page=page;this.changed();this.root.scrollTop=0;});article.append(pages.nav);
    const list=element("ol","","roadmap-items");
    for(const record of records.slice(pages.start,pages.end)){
      const row=element("li"),button=element("button",feedbackTitle(record),"roadmap-item-open");button.dataset.feedbackId=record.id;button.dataset.focusKey="feedback:"+record.id;button.onclick=()=>this.open(record.id);
      row.append(button,element("span",feedbackKind[record.kind]+" · "+feedbackStatus[record.status]+" · "+record.from,"roadmap-item-state"));list.append(row);
    }
    article.append(records.length?list:element("p",this.query.trim()||this.kind!=="all"||this.status!=="all"?"No feedback matches these filters.":"No feedback has been saved."));return article;
  }
}
