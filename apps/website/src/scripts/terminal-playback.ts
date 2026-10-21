import {Terminal} from "@xterm/xterm";
import {FitAddon} from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

type Frame={id:string;terminal:{light:string;dark:string}};
const header="OpenAI Codex\r\ntic-tac-toe/ · Bevy 0.20.0\r\n\r\n";

/** Owns terminal cells only. Project records and playback controls live outside it. */
export class TerminalPlayback{
  private terminal:Terminal;
  private fit=new FitAddon();
  private index=0;
  private revision=0;
  private queue=Promise.resolve();
  private resize:ResizeObserver;
  private themeObserver:MutationObserver;
  private resizeFrame=0;
  constructor(private host:HTMLElement,private frames:Frame[],onKey:(key:string)=>void){
    this.terminal=new Terminal({
      convertEol:true,disableStdin:true,cursorBlink:false,cursorInactiveStyle:"none",
      scrollback:10000,fontFamily:getComputedStyle(host).getPropertyValue("--font-mono").trim(),
      fontSize:13,lineHeight:1.5,screenReaderMode:true,drawBoldTextInBrightColors:false,
    });
    this.terminal.loadAddon(this.fit);
    this.terminal.open(host);
    this.terminal.textarea?.setAttribute("aria-label","Recorded terminal. Left and right arrows step through actions; space plays or pauses.");
    this.terminal.textarea?.setAttribute("readonly","");
    this.terminal.attachCustomKeyEventHandler(event=>{
      if(event.type==="keydown"&&!event.altKey&&!event.metaKey&&!event.ctrlKey&&["ArrowLeft","ArrowRight"," "].includes(event.key)){
        event.preventDefault();onKey(event.key);return false;
      }
      return true;
    });
    this.resize=new ResizeObserver(()=>{
      cancelAnimationFrame(this.resizeFrame);
      this.resizeFrame=requestAnimationFrame(()=>{
        const oldCols=this.terminal.cols,oldRows=this.terminal.rows;
        this.fit.fit();
        if(oldCols!==this.terminal.cols||oldRows!==this.terminal.rows)this.seek(this.index);
      });
    });
    this.resize.observe(host);
    this.themeObserver=new MutationObserver(()=>this.seek(this.index));
    this.themeObserver.observe(document.documentElement,{attributes:true,attributeFilter:["class"]});
  }
  seek(index:number){
    this.index=index;
    const revision=++this.revision;
    this.queue=this.queue.then(async()=>{
      if(revision!==this.revision)return;
      const dark=document.documentElement.classList.contains("dark"),theme=dark?"dark":"light";
      this.terminal.options.theme={
        background:dark?"#101511":"#f8faf8",foreground:dark?"#e1e4e8":"#24292e",
        selectionBackground:dark?"#46564a":"#cddfcf",cursor:dark?"#e1e4e8":"#24292e",
      };
      this.fit.fit();
      this.terminal.reset();
      const write=(text:string)=>new Promise<void>(resolve=>this.terminal.write(text,resolve));
      await write("\x1b[?25l"+header+this.frames.slice(0,index).map(f=>f.terminal[theme]).join(""));
      const activeLine=this.terminal.buffer.active.baseY+this.terminal.buffer.active.cursorY;
      await write(this.frames[index].terminal[theme]);
      if(revision!==this.revision)return;
      this.terminal.scrollToLine(activeLine);
      this.host.dataset.renderedIndex=String(index);
      this.host.dataset.appearance=theme;
    });
    return this.queue;
  }
  dispose(){
    cancelAnimationFrame(this.resizeFrame);this.resize.disconnect();this.themeObserver.disconnect();this.terminal.dispose();
  }
}
