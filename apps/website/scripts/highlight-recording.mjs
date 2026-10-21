import {createHighlighter} from "shiki";
import {structuredPatch} from "diff";
import {codeThemes} from "../src/data/code-theme.mjs";

const highlighter = createHighlighter({
  themes: Object.values(codeThemes),
  langs: ["rust", "toml", "shellscript", "javascript", "json"],
});
const escape = text => String(text).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
export const terminalText = text => String(text).replace(/\r\n?/g, "\n").replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f]/g, "");
const language = path => ({rs:"rust",toml:"toml",js:"javascript",mjs:"javascript",json:"json",sh:"shellscript"})[path?.split(".").at(-1)] ?? "text";
const palette = {
  light:{fg:"#24292e",muted:"#586069",success:"#17633c",error:"#a12622",add:"#e6f5eb",remove:"#ffebe9",hunk:"#365471"},
  dark:{fg:"#e1e4e8",muted:"#abb4bf",success:"#a5d8b0",error:"#ffb1a3",add:"#173927",remove:"#482624",hunk:"#b4c9e3"},
};
const rgb = hex => [1,3,5].map(i=>parseInt(hex.slice(i,i+2),16)).join(";");
const color = (text,hex) => "\x1b[38;2;"+rgb(hex)+"m"+terminalText(text)+"\x1b[39m";
const tokensHtml = tokens => tokens.map(t=>'<span style="'+Object.entries(t.htmlStyle??{}).map(([k,v])=>k+":"+v).join(";")+'">'+escape(t.content)+'</span>').join("");
const tokensAnsi = (tokens,theme) => tokens.map(t=>color(t.content,t.htmlStyle?.[theme==="dark"?"--shiki-dark":"color"]??palette[theme].fg)).join("");
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
    const terminal={};
    for(const theme of ["light","dark"]){
      const p=palette[theme];
      const heading=event.file
        ? "• Edited "+event.file.path+" (+"+diff.added+" −"+diff.removed+")"
        : "• Called magistr."+event.operation;
      const limit=event.file?20:14;
      const body=lines.slice(0,limit).map(line=>{
        const content=line.tokens?tokensAnsi(line.tokens,theme):color(line.text,p[line.kind]??p.fg);
        if(["add","remove"].includes(line.kind)){
          return "\x1b[48;2;"+rgb(p[line.kind])+"m"+color(line.text[0],line.kind==="add"?p.success:p.error)+content+"\x1b[K\x1b[49m";
        }
        return (line.kind==="context"?" ":"")+content;
      }).join("\n");
      terminal[theme]=color(heading,p.fg)+"\n"+color(event.title,p.muted)+"\n\n"+body+
        (lines.length>limit?"\n"+color("… "+(lines.length-limit)+" more lines",p.muted):"")+"\n\n";
    }
    rendered.push({id:event.id,html,diff:diff?{html:diff.html,added:diff.added,removed:diff.removed,first:diff.first}:null,terminal});
  }
  return rendered;
}
