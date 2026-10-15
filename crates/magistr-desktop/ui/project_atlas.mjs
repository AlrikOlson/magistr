import { addressKey } from "./context.mjs";
import { projectGroups, projectVariant } from "./navigation.mjs";
import { sessionActivity } from "./activity.mjs";

// A bounded spatial view of the same catalog. Nodes hold presentation, never session state.
const positions = [[50,14],[82,36],[82,78],[50,90],[18,78],[18,36]];
const node = (tag, name, value="") => { const el=document.createElement(tag);el.className=name;el.textContent=value;return el; };
function place(parent, children) {
  children.forEach((child,index)=>{if(parent.children[index]!==child)parent.insertBefore(child,parent.children[index]??null);});
  while(parent.children.length>children.length)parent.lastChild.remove();
}
export class ProjectAtlas {
  constructor(root,{select,open,label,list}) {
    Object.assign(this,{root,select,open,label,list});this.groups=new Map();this.rows=new Map();
    this.board=node("div","atlas-board");this.more=node("button","atlas-more");
    this.more.onclick=list;root.append(this.board,this.more);
  }
  render(projects,sessions,query="") {
    const all=projectGroups(projects,sessions.filter(session=>!session.closed),query,this.label);
    const groups=all.slice(0,8), used=new Set();
    const cards=groups.map(project=>{
      let card=this.groups.get(project.id);
      if(!card){
        const root=node("section","atlas-project"),heading=node("header","atlas-heading");
        const title=node("h3",""),path=node("p",""),stage=node("nav","atlas-stage");
        const lines=document.createElementNS("http://www.w3.org/2000/svg","svg");
        lines.setAttribute("viewBox","0 0 100 100");lines.setAttribute("preserveAspectRatio","none");lines.setAttribute("aria-hidden","true");
        const hub=node("button","atlas-hub"),mark=node("span","project-monogram"),count=node("span","atlas-hub-count");
        mark.setAttribute("aria-hidden","true");hub.append(mark,count);
        const nodes=node("div","atlas-nodes"),more=node("button","atlas-more");
        more.onclick=this.list;stage.append(lines,hub,nodes);heading.append(title,path);root.append(heading,stage,more);
        card={root,title,path,stage,lines,hub,mark,count,nodes,more,project};this.groups.set(project.id,card);
        hub.onclick=()=>this.open(card.project.directory);
      }
      card.project=project;card.root.dataset.variant=projectVariant(project.id);
      card.title.textContent=project.name;card.path.textContent=project.directory;
      card.mark.textContent=Array.from(project.name.trim()).slice(0,2).join("").toLocaleUpperCase();
      card.count.textContent=project.sessions.length+" open";
      card.hub.setAttribute("aria-label","Open project "+project.name+" from map");
      card.stage.setAttribute("aria-label","Conversation map for "+project.name);
      const visible=project.sessions.slice(0,6), edges=[];
      const controls=visible.map((session,index)=>{
        const key=addressKey(session.address);used.add(key);
        let row=this.rows.get(key);
        if(!row){
          const button=node("button","atlas-node"),name=node("span",""),state=node("small","");
          button.append(name,state);row={button,name,state,session};this.rows.set(key,row);
          button.onclick=()=>this.select(row.session.address);
        }
        row.session=session;row.name.textContent=this.label(session);
        row.state.textContent=session.pending?.length?"Needs your answer":session.status;
        row.button.dataset.state=sessionActivity(session);
        row.button.setAttribute("aria-label","Open "+this.label(session)+" from project map");
        const [x,y]=positions[index];row.button.style.left=x+"%";row.button.style.top=y+"%";
        const edge=document.createElementNS("http://www.w3.org/2000/svg","path");
        edge.setAttribute("d",`M50 52 Q50 ${y} ${x} ${y}`);edges.push(edge);
        return row.button;
      });
      // Only decorative connectors are rebuilt; native button identity and focus survive updates.
      card.lines.replaceChildren(...edges);place(card.nodes,controls);
      card.more.hidden=project.sessions.length<=visible.length;
      card.more.textContent="View all "+project.sessions.length+" conversations in List";
      return card.root;
    });
    place(this.board,cards);
    this.more.hidden=all.length<=groups.length;this.more.textContent="View all "+all.length+" projects in List";
    for(const key of this.rows.keys())if(!used.has(key))this.rows.delete(key);
    for(const key of this.groups.keys())if(!groups.some(project=>project.id===key))this.groups.delete(key);
  }
}
