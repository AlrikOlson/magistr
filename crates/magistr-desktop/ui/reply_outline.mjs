// Contents are derived from the rendered retained document, never a second document store.
export function renderOutline(nav,body){
  const headings=[...body.querySelectorAll("h1,h2,h3")].filter(node=>node.textContent.trim()).slice(0,24);
  nav.hidden=headings.length<3||body.textContent.length<1200;
  if(nav.hidden){nav.replaceChildren();return;}
  const document=nav.ownerDocument;
  if(!nav.firstChild){
    const label=document.createElement("p"),list=document.createElement("ol");
    label.textContent="In this reply";nav.append(label,list);
  }
  const list=nav.lastChild;
  headings.forEach((heading,index)=>{
    let item=list.children[index];
    if(!item){item=document.createElement("li");item.append(document.createElement("button"));list.append(item);}
    const button=item.firstChild;button.type="button";button.textContent=heading.textContent;button.dataset.level=heading.localName;
    button.onclick=()=>{heading.tabIndex=-1;heading.focus({preventScroll:true});heading.scrollIntoView({block:"start"});};
  });
  while(list.children.length>headings.length)list.lastChild.remove();
}
