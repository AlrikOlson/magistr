// Codex history-cell conventions: a two-column activity gutter, four-column
// tool results, hanging continuations, and numbered patch lines. The recording
// owns content; this module owns presentation at the fitted terminal width.
export const terminalPalette = {
  light:{fg:"#24292e",muted:"#586069",accent:"#0550ae",success:"#17633c",error:"#a12622",add:"#e6f5eb",remove:"#ffebe9",hunk:"#365471"},
  dark:{fg:"#e1e4e8",muted:"#abb4bf",accent:"#a6c8ff",success:"#a5d8b0",error:"#ffb1a3",add:"#173927",remove:"#482624",hunk:"#b4c9e3"},
};
export const terminalText = text => String(text).replace(/\r\n?/g,"\n").replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f]/g,"").replace(/\t/g,"    ");
const rgb = hex => [1,3,5].map(i=>parseInt(hex.slice(i,i+2),16)).join(";");
const escape = text => String(text).replaceAll("&","&amp;").replaceAll("<","&lt;").replaceAll(">","&gt;").replaceAll('"',"&quot;");
const color = (span,theme) => span[theme]??terminalPalette[theme][span.tone??"fg"]??terminalPalette[theme].fg;
const ansi = (span,theme) => (span.bold?"\x1b[1m":"")+"\x1b[38;2;"+rgb(color(span,theme))+"m"+span.text+"\x1b[0m";

// Keep color boundaries out of wrapping decisions. In particular, a highlighted
// identifier is still one word even when the highlighter emits several spans.
function wrapRow(row,columns){
  const cells=row.spans.flatMap(span=>Array.from(terminalText(span.text),text=>({...span,text})));
  const result=[];let start=0,prefix=row.prefix;
  do{
    const capacity=Math.max(1,columns-prefix.length);
    let end=Math.min(cells.length,start+capacity),next=end;
    if(end<cells.length){
      let space=end;
      while(space>start&&cells[space]?.text!==" ")space--;
      // Leading source indentation is not a word boundary.
      if(space>start&&cells.slice(start,space).some(cell=>cell.text!==" ")){
        end=space;next=space+1;
        while(next<cells.length&&cells[next].text===" ")next++;
      }
    }
    const spans=[];
    for(const cell of cells.slice(start,end)){
      const last=spans.at(-1);
      if(last&&last.light===cell.light&&last.dark===cell.dark&&last.tone===cell.tone&&last.bold===cell.bold)last.text+=cell.text;
      else spans.push({...cell});
    }
    result.push({prefix,spans,background:row.background,heading:row.heading});
    start=next;prefix=row.continuation;
  }while(start<cells.length);
  return result;
}
export function formatTerminalFrame(frame,columns,theme,maxRows=24){
  // Leave a column free so xterm never inserts an unindented automatic wrap.
  const width=Math.max(12,columns-1);
  let rows=frame.rows.flatMap(row=>wrapRow(row,width));
  if(rows.length>maxRows)rows=[...rows.slice(0,maxRows-1),{prefix:"    ",spans:[{text:"… more in details",tone:"muted"}]}];
  return rows.map(row=>{
    const p=terminalPalette[theme];
    const prefix=ansi({text:row.prefix,tone:row.background==="add"?"success":row.background==="remove"?"error":"muted"},theme);
    const background=row.background?"\x1b[48;2;"+rgb(p[row.background])+"m":"";
    // SGR resets must restore the line's background between syntax tokens.
    const body=row.spans.map(span=>background+ansi(span,theme)).join("");
    const fill=background?background+" ".repeat(Math.max(0,width-row.prefix.length-row.spans.reduce((n,s)=>n+Array.from(s.text).length,0))):"";
    return prefix+body+fill+"\x1b[0m";
  }).join("\r\n")+"\r\n\r\n";
}
export function terminalFrameHTML(frame){
  return frame.rows.map(row=>'<div class="terminal-row'+(row.heading?' terminal-heading':'')+(row.background?' terminal-'+row.background:'')+'" style="--gutter:'+row.prefix.length+'ch;--hanging:'+(row.continuation.length-row.prefix.length)+'ch"><span class="terminal-gutter" aria-hidden="true">'+escape(row.prefix)+'</span><span>'+row.spans.map(span=>'<span style="color:'+color(span,"light")+';--terminal-dark:'+color(span,"dark")+(span.bold?';font-weight:600':'')+'">'+escape(span.text)+'</span>').join("")+'</span></div>').join("");
}
