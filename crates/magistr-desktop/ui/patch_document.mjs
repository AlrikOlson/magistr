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
