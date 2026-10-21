import {Terminal} from "@xterm/xterm";
import {FitAddon} from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {formatTerminalFrame} from "../data/terminal-format.mjs";

type Frame={id:string;terminal:Parameters<typeof formatTerminalFrame>[0]};

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
  private formatKey="";
  private formatted:string[]=[];
  constructor(private host:HTMLElement,private frames:Frame[],onKey:(key:string)=>void){
    this.terminal=new Terminal({
      convertEol:true,disableStdin:true,cursorBlink:false,cursorInactiveStyle:"none",
      scrollback:10000,fontFamily:getComputedStyle(host).getPropertyValue("--terminal-font").trim(),
      fontSize:13,lineHeight:1.35,screenReaderMode:true,drawBoldTextInBrightColors:false,
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
      const key=this.terminal.cols+":"+this.terminal.rows+":"+theme;
      if(key!==this.formatKey){this.formatKey=key;this.formatted=[];}
      const frame=(i:number)=>this.formatted[i]??=formatTerminalFrame(this.frames[i].terminal,this.terminal.cols,theme,this.terminal.rows-2);
      this.terminal.reset();
      const write=(text:string)=>new Promise<void>(resolve=>this.terminal.write(text,resolve));
      await write("\x1b[?25l"+this.frames.slice(0,index).map((_,i)=>frame(i)).join(""));
      const activeLine=this.terminal.buffer.active.baseY+this.terminal.buffer.active.cursorY;
      const current=frame(index),lineCount=current.split("\n").length-1;
      // Fill the selected screen, keeping earlier actions in real scrollback.
      await write(current+"\r\n".repeat(Math.max(0,this.terminal.rows-lineCount)));
      if(revision!==this.revision)return;
      this.terminal.scrollToLine(activeLine);
      this.host.dataset.renderedIndex=String(index);
      this.host.dataset.appearance=theme;
      this.host.dataset.columns=String(this.terminal.cols);
    });
    return this.queue;
  }
  dispose(){
    cancelAnimationFrame(this.resizeFrame);this.resize.disconnect();this.themeObserver.disconnect();this.terminal.dispose();
  }
}
