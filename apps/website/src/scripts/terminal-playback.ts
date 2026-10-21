import {Terminal} from "@xterm/xterm";
import {FitAddon} from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {formatTerminalFrame,terminalStreamParts} from "../data/terminal-format.mjs";

type Frame={id:string;terminal:Parameters<typeof formatTerminalFrame>[0]&{role?:string}};
type Stream={parts:string[];offset:number;speed:number;paused:boolean;writing:boolean;done:Promise<boolean>;resolve:(ok:boolean)=>void;timer?:ReturnType<typeof setTimeout>;revision:number};

/** Owns terminal cells and reveal progress. The walkthrough owns the timeline. */
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
  private stream?:Stream;
  private reduced=matchMedia("(prefers-reduced-motion: reduce)");
  constructor(private host:HTMLElement,private frames:Frame[],onKey:(key:string)=>void,private onReflow:()=>void){
    this.terminal=new Terminal({
      convertEol:true,disableStdin:true,cursorBlink:false,cursorInactiveStyle:"none",
      scrollback:10000,fontFamily:getComputedStyle(host).getPropertyValue("--terminal-font").trim(),
      fontSize:13,lineHeight:1.35,screenReaderMode:true,drawBoldTextInBrightColors:false,
    });
    this.terminal.loadAddon(this.fit);
    this.terminal.open(host);
    this.terminal.textarea?.setAttribute("aria-label","Recorded terminal. Left and right arrows step through the session; space plays or pauses.");
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
        if(oldCols!==this.terminal.cols||oldRows!==this.terminal.rows)this.reflow();
      });
    });
    this.resize.observe(host);
    this.themeObserver=new MutationObserver(()=>{
      const appearance=document.documentElement.classList.contains("dark")?"dark":"light";
      if(this.host.dataset.appearance&&this.host.dataset.appearance!==appearance)this.reflow();
    });
    this.themeObserver.observe(document.documentElement,{attributes:true,attributeFilter:["class"]});
    this.reduced.addEventListener("change",this.reflow);
  }
  private reflow=()=>{this.onReflow();void this.seek(this.index);};
  private write(text:string){return new Promise<void>(resolve=>this.terminal.write(text,resolve));}
  private cancel(){
    if(this.stream){clearTimeout(this.stream.timer);this.stream.resolve(false);this.stream=undefined;}
    this.host.dataset.streaming="false";
  }
  pause(){
    if(this.stream){this.stream.paused=true;clearTimeout(this.stream.timer);}
    this.host.dataset.streaming="false";
  }
  setSpeed(speed:number){if(this.stream)this.stream.speed=speed;}
  seek(index:number){return this.present(index,false,1);}
  play(index:number,speed:number){
    if(index===this.index&&this.stream){
      this.stream.speed=speed;
      if(this.stream.paused){this.stream.paused=false;this.tick(this.stream);}
      return this.stream.done;
    }
    return this.present(index,!this.reduced.matches,speed);
  }
  private tick(stream:Stream){
    if(stream!==this.stream||stream.paused||stream.writing||stream.revision!==this.revision)return;
    if(stream.offset===stream.parts.length){
      this.host.dataset.streaming="false";stream.resolve(true);return;
    }
    this.host.dataset.streaming="true";
    const offset=++stream.offset;stream.writing=true;
    // Writes are acknowledged before scheduling another chunk, so pause/seek
    // cannot leave a backlog of old text queued inside xterm.
    void this.write(stream.parts[offset-1]).then(()=>{
      if(stream!==this.stream||stream.revision!==this.revision)return;
      stream.writing=false;this.host.dataset.revealed=String(offset);
      this.terminal.scrollToBottom();
      if(!stream.paused)stream.timer=setTimeout(()=>this.tick(stream),(this.frames[this.index].terminal.role?24:75)/stream.speed);
    });
  }
  private present(index:number,animate:boolean,speed:number):Promise<boolean>{
    this.cancel();this.index=index;
    const revision=++this.revision;
    let resolve!:(ok:boolean)=>void;
    const done=new Promise<boolean>(r=>{resolve=r;});
    const stream:Stream={parts:[],offset:0,speed,paused:false,writing:true,done,resolve,revision};
    this.stream=stream;
    this.queue=this.queue.then(async()=>{
      if(revision!==this.revision){resolve(false);return;}
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
      await this.write("\x1b[?25l"+this.frames.slice(0,index).map((_,i)=>frame(i)).join(""));
      if(revision!==this.revision){resolve(false);return;}
      const activeLine=this.terminal.buffer.active.baseY+this.terminal.buffer.active.cursorY;
      const current=frame(index),lineCount=current.split("\n").length-1;
      // Leave a little conversation context above a short message. Long output
      // starts at its own beginning and remains available in real scrollback.
      const context=this.frames[index].terminal.role?Math.min(6,Math.max(0,this.terminal.rows-lineCount)):0;
      const anchor=Math.max(0,activeLine-context);
      const markRendered=()=>{this.host.dataset.renderedIndex=String(index);this.host.dataset.appearance=theme;this.host.dataset.columns=String(this.terminal.cols);};
      this.host.dataset.revealed="0";
      if(!animate||this.frames[index].terminal.role==="user"){
        await this.write(current+"\r\n".repeat(Math.max(0,this.terminal.rows-lineCount)));
        if(revision!==this.revision){resolve(false);return;}
        this.terminal.scrollToLine(anchor);markRendered();stream.resolve(true);
        // A seek is a completed still; a later Play reveals this entry anew.
        this.stream=undefined;
        return;
      }
      // Reserve a stable screen and put the cursor back where the action starts.
      await this.write("\r\n".repeat(this.terminal.rows-context)+"\x1b["+(this.terminal.rows-context)+"A");
      if(revision!==this.revision){resolve(false);return;}
      this.terminal.scrollToLine(anchor);markRendered();stream.writing=false;
      stream.parts=terminalStreamParts(current,this.frames[index].terminal.role);
      this.tick(stream);
    }).catch(error=>{this.cancel();resolve(false);throw error;});
    return done;
  }
  dispose(){
    this.cancel();++this.revision;cancelAnimationFrame(this.resizeFrame);
    this.resize.disconnect();this.themeObserver.disconnect();
    this.reduced.removeEventListener("change",this.reflow);this.terminal.dispose();
  }
}
