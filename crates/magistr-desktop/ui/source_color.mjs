// Bounded lexical color for explicitly named languages. Source bytes remain authoritative.
const aliases = {js:"javascript",ts:"javascript",typescript:"javascript",jsx:"javascript",tsx:"javascript",rs:"rust",py:"python",sh:"shell",bash:"shell"};
const keywords = {
  javascript: "async await break case catch class const continue debugger default delete do else export extends finally for from function if import in instanceof let new of return static super switch this throw try typeof var void while with yield",
  rust: "as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while",
  python: "and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield",
  shell: "if then else elif fi for while do done case esac function in export local",
  json: "",
};
const literals = new Set(["true","false","null","undefined","None","True","False"]);
const cache = new Map(Object.entries(keywords).map(([name, words]) => [name,new Set(words.split(" "))]));
export function colorSource(text, language) {
  const name = aliases[language?.toLowerCase()] ?? language?.toLowerCase();
  if (!cache.has(name) || text.length > 65536) return [{text,kind:""}];
  const words = cache.get(name), hash = name === "python" || name === "shell";
  const tokens = [];
  const push = (start, end, kind) => {
    const previous=tokens.at(-1), value=text.slice(start,end);
    if (previous?.kind===kind) previous.text+=value; else tokens.push({text:value,kind});
  };
  let index=0;
  while(index<text.length) {
    const start=index, char=text[index]; let kind="";
    if ((hash && char==="#") || (!hash && text.startsWith("//",index))) {
      const end=text.indexOf("\n",index);index=end<0?text.length:end;kind="comment";
    } else if (!hash && text.startsWith("/*",index)) {
      const end=text.indexOf("*/",index+2);index=end<0?text.length:end+2;kind="comment";
    } else if (char==='"' || char==="'" || (char==="\x60" && name==="javascript")) {
      // Rust lifetimes are identifiers, while its single-quoted characters have a short closing delimiter.
      if(name==="rust" && char==="'" && !/^'(?:\\.|[^'\\\n])'/u.test(text.slice(index,index+12))) {index++;}
      else {
        const delimiter=name==="python" && text.startsWith(char.repeat(3),index)?char.repeat(3):char;
        index+=delimiter.length;
        while(index<text.length) {
          if(text[index]==="\\") {index=Math.min(index+2,text.length);continue;}
          if(text.startsWith(delimiter,index)) {index+=delimiter.length;break;}
          index++;
        }
        kind="string";
      }
    } else if (/[A-Za-z_$]/.test(char)) {
      index++;while(index<text.length && /[\w$]/.test(text[index]))index++;
      const word=text.slice(start,index);kind=literals.has(word)?"literal":words.has(word)?"keyword":"";
    } else if (/\d/.test(char)) {
      index++;while(index<text.length && /[\d._]/.test(text[index]))index++;kind="literal";
    } else {index++;}
    push(start,index,kind);
    if(tokens.length>12000)return [{text,kind:""}];
  }
  return tokens;
}
export function paintSource(node, text, language) {
  if (node.textContent === text && node.dataset.language === (language ?? "")) return;
  const document = node.ownerDocument, fragment = document.createDocumentFragment();
  for (const token of colorSource(text,language)) {
    if (!token.kind) fragment.append(document.createTextNode(token.text));
    else { const span=document.createElement("span");span.className="source-"+token.kind;span.textContent=token.text;fragment.append(span); }
  }
  node.replaceChildren(fragment); node.dataset.language=language ?? "";
}
