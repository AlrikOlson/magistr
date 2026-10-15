// Contents are derived from the rendered retained document, never a second document store.
export function renderOutline(nav,body){
  const headings=[...body.querySelectorAll("h1,h2,h3")].filter(node=>node.textContent.trim()).slice(0,24);
  nav.hidden=headings.length<3||body.textContent.length<1200;
  if(nav.hidden){nav.replaceChildren();return;}
  const document=nav.ownerDocument;
  if(!nav.firstChild){
    const toggle=document.createElement("button"),list=document.createElement("ol");
    toggle.type="button";toggle.className="reply-outline-toggle";toggle.setAttribute("aria-label","Reply sections");toggle.setAttribute("aria-expanded","false");
    toggle.onclick=()=>{const open=nav.classList.toggle("outline-open");toggle.setAttribute("aria-expanded",String(open));};
    nav.append(toggle,list);
  }
  const list=nav.lastChild;nav.querySelector(".reply-outline-toggle").textContent="In this reply · "+headings.length+" sections";
  headings.forEach((heading,index)=>{
    let item=list.children[index];
    if(!item){item=document.createElement("li");item.append(document.createElement("button"));list.append(item);}
    const button=item.firstChild;button.type="button";button.textContent=heading.textContent;button.dataset.level=heading.localName;
    button.onclick=()=>{nav.classList.remove("outline-open");nav.querySelector(".reply-outline-toggle").setAttribute("aria-expanded","false");heading.tabIndex=-1;heading.focus({preventScroll:true});heading.scrollIntoView({block:"start"});};
  });
  while(list.children.length>headings.length)list.lastChild.remove();
}