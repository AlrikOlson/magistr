// A bounded projection of explicitly labelled unified patch text; never repository state.
export function patchLines(text){
  if(text.length>65536)return null;
  const lines=text.match(/[^\n]*\n|[^\n]+$/g)??[];
  if(lines.length>2000)return null;
  let before=0,after=0,left=0,right=0,added=0,removed=0;
  const rows=lines.map(text=>{
    let kind="context",old="",next="";
    const hunk=/^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/.exec(text);
    if(hunk){
      before=Number(hunk[1]);after=Number(hunk[3]);left=Number(hunk[2]??1);right=Number(hunk[4]??1);
      if(![before,after,left,right].every(Number.isSafeInteger)){left=0;right=0;}
      kind="hunk";
    } else if(left>0||right>0){
      if(text.startsWith("+")&&right>0){kind="added";next=after++;right--;added++;}
      else if(text.startsWith("-")&&left>0){kind="removed";old=before++;left--;removed++;}
      else if(text.startsWith(" ")&&left>0&&right>0){old=before++;next=after++;left--;right--;}
      else if(!text.startsWith("\\")){left=0;right=0;kind="file";}
    } else if(text.startsWith("diff ")||text.startsWith("--- ")||text.startsWith("+++ ")||text.startsWith("index "))kind="file";
    return {text,kind,old,next};
  });
  return {rows,added,removed};
}
export function paintPatch(node,text){
  const patch=patchLines(text);if(!patch)return null;
  if(node.textContent!==text||node.dataset.language!=="patch-view"){
    const document=node.ownerDocument,fragment=document.createDocumentFragment();
    for(const row of patch.rows){
      const line=document.createElement("span");line.className="patch-line patch-"+row.kind;
      line.dataset.position=String(row.old).padStart(5)+" "+String(row.next).padStart(5);
      line.textContent=row.text;fragment.append(line);
    }
    node.replaceChildren(fragment);node.dataset.language="patch-view";
  }
  return patch;
}

// Pair adjacent removal/addition runs by position within each hunk, never across hunks.
export function pairPatchRows(patch) {
  const result=[],removed=[],added=[];
  const flush=()=>{
    for(let i=0;i<Math.max(removed.length,added.length);i++)result.push({before:removed[i]??null,after:added[i]??null});
    removed.length=0;added.length=0;
  };
  for(const row of patch.rows){
    if(row.kind==="removed"){removed.push(row);continue;}
    if(row.kind==="added"){added.push(row);continue;}
    flush();
    if(row.kind==="context"&&row.old!==""&&row.next!=="")result.push({before:row,after:row});
    else result.push({label:row.text,kind:row.kind});
  }
  flush();return result;
}
export function paintPatchComparison(root,patch) {
  const document=root.ownerDocument,fragment=document.createDocumentFragment();
  for(const title of ["Before","After"]){const heading=document.createElement("div");heading.className="diff-column-title";heading.textContent=title;fragment.append(heading);}
  for(const row of pairPatchRows(patch)){
    if(row.label!==undefined){const label=document.createElement("div");label.className="diff-pair-label";label.textContent=row.label;fragment.append(label);continue;}
    for(const [side,line] of [["before",row.before],["after",row.after]]){
      const cell=document.createElement("div");cell.className="diff-cell"+(line?" diff-cell-"+line.kind:" diff-cell-empty");
      const number=document.createElement("span");number.className="diff-line-number";number.setAttribute("aria-hidden","true");
      number.textContent=line?String(side==="before"?line.old:line.next):"";
      const source=document.createElement("code");source.textContent=line?line.text.slice(1):"";cell.append(number,source);fragment.append(cell);
    }
  }
  root.replaceChildren(fragment);
}
