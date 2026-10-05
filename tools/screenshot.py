#!/usr/bin/env python3
"""Draw a tmux pane as a PNG: how the screenshots in assets/ are made.

Usage: tools/screenshot.py <tmux socket name> <output.png> [background r,g,b]

Run the game in a detached tmux session first, e.g.

    tmux -L shot new-session -d -x 150 -y 46 "COLORTERM=truecolor ./wordl"
    tmux -L shot send-keys -l slate; tmux -L shot send-keys Enter
    tools/screenshot.py shot assets/screenshot.png

Each cell is drawn from what tmux reports (colors, block characters, text), so the
picture shows the game's real output without needing a terminal window. Needs tmux and
rsvg-convert (librsvg).
"""
import html
import re
import subprocess
import sys

sock, out = sys.argv[1], sys.argv[2]
out = out[:-4] if out.endswith('.png') else out
data=subprocess.run(['tmux','-L',sock,'capture-pane','-p','-e'],capture_output=True,text=True).stdout
CW,CH=9,18
ANSI=[(0,0,0),(205,49,49),(13,188,121),(229,229,16),(36,114,200),(188,63,188),(17,168,205),(229,229,229),(102,102,102),(241,76,76),(35,209,139),(245,245,67),(59,142,234),(214,112,214),(41,184,219),(255,255,255)]
def c256(n):
    if n<16: return ANSI[n]
    if n<232:
        n-=16; f=lambda v:0 if v==0 else 55+40*v
        return (f(n//36),f(n//6%6),f(n%6))
    v=8+10*(n-232); return (v,v,v)
DFG,DBG=(220,220,220),tuple(int(v) for v in (sys.argv[3] if len(sys.argv)>3 else "17,19,26").split(","))
rects=[];texts=[]
lines=data.split('\n')
if lines and lines[-1]=='': lines.pop()
for y,line in enumerate(lines):
    fg,bg,bold=DFG,DBG,False; x=0; i=0
    while i<len(line):
        m=re.match(r'\x1b\[([0-9;:]*)m',line[i:])
        if m:
            p=[int(v) if v else 0 for v in re.split('[;:]',m.group(1))] or [0]
            j=0
            while j<len(p):
                v=p[j]
                if v==0: fg,bg,bold=DFG,DBG,False
                elif v==1: bold=True
                elif v==22: bold=False
                elif v in(38,48):
                    if p[j+1]==2: col=tuple(p[j+2:j+5]); j+=4
                    else: col=c256(p[j+2]); j+=2
                    if v==38: fg=col
                    else: bg=col
                elif 30<=v<=37: fg=ANSI[v-30]
                elif v==39: fg=DFG
                elif 40<=v<=47: bg=ANSI[v-40]
                elif v==49: bg=DBG
                elif 90<=v<=97: fg=ANSI[v-82]
                elif 100<=v<=107: bg=ANSI[v-92]
                j+=1
            i+=len(m.group(0)); continue
        m=re.match(r'\x1b[\(\)].|\x0f|\x0e',line[i:])
        if m: i+=len(m.group(0)); continue
        ch=line[i]; i+=1
        X,Y=x*CW,y*CH
        rects.append((X,Y,CW,CH,bg))
        if ch=='█': rects.append((X,Y,CW,CH,fg))
        elif ch=='▀': rects.append((X,Y,CW,CH//2,fg))
        elif ch=='▄': rects.append((X,Y+CH//2,CW,CH//2,fg))
        elif ch=='━': rects.append((X,Y+CH//2-1,CW,3,fg))
        elif ch!=' ': texts.append((X+CW/2,Y+CH*0.75,ch,fg,bold))
        x+=1
W=int(subprocess.run(["tmux","-L",sock,"display","-p","#{window_width}"],capture_output=True,text=True).stdout)*CW; H=len(lines)*CH
s=[f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}"><rect width="100%" height="100%" fill="rgb{DBG}"/>']
for X,Y,w,h,c in rects: s.append(f'<rect x="{X}" y="{Y}" width="{w}" height="{h}" fill="rgb{c}" shape-rendering="crispEdges"/>')
for X,Y,ch,c,b in texts: s.append(f'<text x="{X}" y="{Y}" text-anchor="middle" font-family="JetBrainsMono Nerd Font" font-size="14" font-weight="{"bold" if b else "normal"}" fill="rgb{c}">{html.escape(ch)}</text>')
s.append('</svg>')
open(out+'.svg','w').write('\n'.join(s))
subprocess.run(['rsvg-convert',out+'.svg','-o',out+'.png'],check=True)
import os
os.remove(out+'.svg')
