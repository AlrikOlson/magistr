import {createHighlighter} from "shiki";
import {structuredPatch} from "diff";
import {codeThemes} from "../src/data/code-theme.mjs";
import {terminalText} from "../src/data/terminal-format.mjs";

const highlighter = createHighlighter({
  themes: Object.values(codeThemes),
  langs: ["rust", "toml", "shellscript", "javascript", "json"],
});
const escape = text => String(text).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
const language = path => ({rs:"rust",toml:"toml",js:"javascript",mjs:"javascript",json:"json",sh:"shellscript"})[path?.split(".").at(-1)] ?? "text";
const tokensHtml = tokens => tokens.map(t=>'<span style="'+Object.entries(t.htmlStyle??{}).map(([k,v])=>k+":"+v).join(";")+'">'+escape(t.content)+'</span>').join("");
async function tokenLines(text,lang){return (await highlighter).codeToTokens(text,{lang,themes:codeThemes}).tokens;}
export async function highlightCode(text,lang="text"){
  return (await highlighter).codeToHtml(text,{lang,themes:codeThemes});
}

export async function recordedDiff(before,after,path){
  const patch=structuredPatch(path,path,before,after,"","",{context:3});
  const lang=language(path),[oldTokens,newTokens]=await Promise.all([tokenLines(before,lang),tokenLines(after,lang)]);
  const lines=[];let added=0,removed=0;
  for(const h of patch.hunks){
    lines.push({kind:"hunk",text:`@@ -${h.oldLines?h.oldStart:h.oldStart-1},${h.oldLines} +${h.newLines?h.newStart:h.newStart-1},${h.newLines} @@`});
    let oldLine=h.oldStart,newLine=h.newStart;
    for(const line of h.lines){
      const prefix=line[0],text=line.slice(1);
      if(prefix==="\\"){lines.push({kind:"meta",text:line});continue;}
      const kind=prefix==="+"?"add":prefix==="-"?"remove":"context";
      const tokens=(kind==="remove"?oldTokens[oldLine-1]:newTokens[newLine-1])??[{content:text}];
      lines.push({kind,text:prefix+text,tokens,oldLine:kind==="add"?null:oldLine,newLine:kind==="remove"?null:newLine});
      if(kind!=="add")oldLine++;if(kind!=="remove")newLine++;
      if(kind==="add")added++;if(kind==="remove")removed++;
    }
  }
  const html='<pre class="code-diff" tabindex="0" aria-label="Changes to '+escape(path)+'"><code>'+lines.map(line=>{
    if(!line.tokens)return '<span class="diff-line diff-'+line.kind+'">'+escape(line.text)+'</span>';
    return '<span class="diff-line diff-'+line.kind+'"><span class="diff-sign">'+escape(line.text[0])+'</span>'+tokensHtml(line.tokens)+'</span>';
  }).join("\n")+'</code></pre>';
  return {lines,html,added,removed};
}
function outputKind(line){
  if(/\b(error|panicked|FAILED|failures:)\b|\[exit [1-9]|test result: FAILED/.test(line))return "error";
  if(/(?:\.\.\. ok|test result: ok|passed\.|^PASS\b|\[exit 0)/.test(line))return "success";
  if(/^\s*(Compiling|Finished|Running)|^\[exit /.test(line))return "muted";
  return "";
}
async function checkLines(event){
  const command=event.check?.command;
  const prefix=command?"$ "+command:"";
  const count=prefix&&event.output.startsWith(prefix)?prefix.split("\n").length:0;
  const commandTokens=count?await tokenLines(prefix,"shellscript"):[];
  const output=event.output.split("\n");
  const snippets=output.map((text,i)=>i>=count&&/\.rs:/.test(event.output)?text.match(/^(\s*\d+\s+[|~](?: [|/])?\s)(.*)$/):null);
  const rustTokens=snippets.some(Boolean)?await tokenLines(snippets.map(match=>match?.[2]??"").join("\n"),"rust"):[];
  return output.map((text,i)=>({
    text,
    tokens:i<count?commandTokens[i]:snippets[i]?[{content:snippets[i][1]},...rustTokens[i]]:undefined,
    kind:i<count?"command":snippets[i]?"code":outputKind(text),
  }));
}
function outputHtml(lines){
  return '<pre class="code-output" tabindex="0"><code>'+lines.map(line=>'<span class="output-line'+(line.kind?' output-'+line.kind:'')+'">'+(line.tokens?tokensHtml(line.tokens):escape(line.text))+'</span>').join("\n")+'</code></pre>';
}
export async function renderRecording(recording){
  const files=new Map(),rendered=[];
  for(const event of recording.events){
    if(event.kind==="message"){
      const user=event.role==="user";
      const rows=event.output.split("\n").map((text,i)=>({
        prefix:i===0?(user?"› ":"• "):"  ",continuation:"  ",
        ...(user?{background:"user"}:{}),
        spans:[{text:terminalText(text.replace(/\[([^\]]+)\]\([^)]+\)/g,"$1").replace(/\*\*([^*]+)\*\*/g,"$1").replace(/`([^`]+)`/g,"$1")),tone:"fg"}],
      }));
      rendered.push({id:event.id,html:'<pre class="conversation-message"><code>'+escape(event.output)+'</code></pre>',diff:null,terminal:{role:event.role,rows}});
      continue;
    }
    let diff,lines,html;
    if(event.file){
      const before=files.get(event.file.path)??"";
      diff=await recordedDiff(before,event.file.content,event.file.path);
      diff.first=!files.has(event.file.path);
      files.set(event.file.path,event.file.content);
      lines=diff.lines;
      html=await highlightCode(event.file.content,language(event.file.path));
    }else{
      lines=await checkLines(event);
      html=outputHtml(lines);
    }
    const terminal={rows:[]};
    const span=(text,tone="muted",bold=false)=>({text:terminalText(text),tone,...(bold?{bold}: {})});
    terminal.rows.push({prefix:"• ",continuation:"  ",heading:true,spans:event.file
      ? [span("Edited ","fg",true),span(event.file.path,"fg"),span(" (+"+diff.added+" -"+diff.removed+")")]
      : [span("Called ","fg",true),span("magistr."+event.operation,"accent")]});
    const displayLines=event.file?lines.filter(line=>line.kind!=="hunk"):lines;
    while(displayLines.at(-1)?.text==="")displayLines.pop();
    const numberWidth=event.file?String(Math.max(1,...displayLines.map(l=>l.newLine??l.oldLine??0))).length:0;
    const limit=event.file?20:14;
    for(const [index,line]of displayLines.slice(0,limit).entries()){
      const tone=line.kind==="error"||line.kind==="success"?line.kind:"muted";
      const spans=line.tokens?line.tokens.map(t=>({
        text:terminalText(t.content),light:t.htmlStyle?.color,dark:t.htmlStyle?.["--shiki-dark"],
      })):[span(line.text,tone)];
      const numbered=event.file&&line.tokens;
      const prefix=numbered
        ? "    "+String(line.newLine??line.oldLine).padStart(numberWidth)+" "+line.text[0]
        : index===0?"  └ ":"    ";
      // Wrapped list items align under their text. Source indentation stays in
      // its original tokens; the continuation gutter does not repeat a number.
      const listIndent=!event.file?line.text.match(/^(\s*(?:[-*•]|\d+\.) )/)?.[0].length??0:0;
      terminal.rows.push({prefix,continuation:" ".repeat(prefix.length+listIndent),spans,
        ...(["add","remove"].includes(line.kind)?{background:line.kind}:{})});
    }
    if(displayLines.length>limit)terminal.rows.push({prefix:"    ",continuation:"    ",spans:[span("… more in details")]});
    rendered.push({id:event.id,html,diff:diff?{html:diff.html,added:diff.added,removed:diff.removed,first:diff.first}:null,terminal});
  }
  return rendered;
}
