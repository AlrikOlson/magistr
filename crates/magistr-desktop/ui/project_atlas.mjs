import {addressKey} from "./context.mjs";
import {projectGroups} from "./navigation.mjs";
import {sessionActivity} from "./activity.mjs";
import {element} from "./work_dom.mjs";
import {WorkSurface} from "./work_surface.mjs";

// Membership comes from the catalog; positions and camera are only local view state.
export class ProjectAtlas{
  constructor(root,{select,work,open,label,list}){
    Object.assign(this,{root,select,work,open,label,list});this.controls=new Map();
    this.surface=new WorkSurface("Projects");this.more=element("button","","atlas-more");this.more.onclick=list;
    root.append(this.surface.root,this.more);
  }
  piece(id,kind,label,detail,footer){
    let button=this.controls.get(id);
    if(!button){button=element("button","","atlas-piece atlas-"+kind);button.append(element("strong"),element("span","","atlas-piece-detail"),element("small"));this.controls.set(id,button);}
    button.children[0].textContent=label;button.children[1].textContent=detail;button.children[2].textContent=footer;
    button.title=[label,detail,footer].filter(Boolean).join("\n");return button;
  }
  render(projects,sessions,query=""){
    const all=projectGroups(projects,sessions.filter(session=>!session.closed),query,this.label),groups=all.slice(0,8),nodes=[],edges=[],actions=new Map();
    const focused=this.root.contains(document.activeElement)?document.activeElement.dataset.focusKey:null;
    let y=0;
    for(const project of groups){
      const id="project:"+project.id,visible=project.sessions.slice(0,6),extra=project.sessions.length>visible.length;
      const rows=visible.length+Number(extra),height=Math.max(104,rows*88-12);
      const button=this.piece(id,"project",project.name,project.directory,project.sessions.length?"Open workspace · "+project.sessions.length+" "+(project.sessions.length===1?"conversation":"conversations"):"Open project");
      button.setAttribute("aria-label",(project.sessions.length?"Open workspace for ":"Open project ")+project.name+" from map");
      nodes.push({id,label:project.name,x:0,y:y+(height-104)/2,w:260,h:104,element:button});
      actions.set(id,()=>project.sessions.length?this.work(project,"roadmap",button):this.open(project.directory));
      visible.forEach((session,index)=>{
        const child="conversation:"+addressKey(session.address),label=this.label(session),state=session.pending?.length?"Needs your answer":session.status;
        const piece=this.piece(child,"conversation",label,state,"Open conversation");piece.dataset.state=sessionActivity(session);
        piece.setAttribute("aria-label","Open "+label+" from project map");
        nodes.push({id:child,label,x:376,y:y+index*88,w:280,h:76,element:piece});
        edges.push({from:id,to:child,kind:"membership",label:label+" belongs to "+project.name});
        actions.set(child,()=>this.select(session.address));
      });
      if(extra){
        const child=id+":more",label="View all "+project.sessions.length+" conversations in List",piece=this.piece(child,"more",String(project.sessions.length-visible.length)+" more conversations",project.name,"View in List");
        piece.setAttribute("aria-label",label);nodes.push({id:child,label,x:376,y:y+visible.length*88,w:280,h:76,element:piece});
        edges.push({from:id,to:child,kind:"membership",label:"More conversations in "+project.name});actions.set(child,this.list);
      }
      y+=height+48;
    }
    this.surface.update({key:"projects:"+query,nodes,edges,focusId:nodes[0]?.id,onSelect:id=>actions.get(id)?.()});
    this.more.hidden=all.length<=groups.length;this.more.textContent="View all "+all.length+" projects in List";
    for(const id of this.controls.keys())if(!actions.has(id))this.controls.delete(id);
    if(focused)this.root.querySelector('[data-focus-key="'+CSS.escape(focused)+'"]')?.focus({preventScroll:true});
  }
}