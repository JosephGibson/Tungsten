"""HUD hearts, burning-ball flame frames, the explosion shock ring and the iron brick."""
import math

from PIL import Image

from art import PALETTE, canvas, rng_for


def enrich_art(images,lit,animations):
    heart=[(8,15),(15,8),(25,8),(32,15),(39,8),(49,8),(56,15),(56,31),(32,56),(8,31)]
    for name in ('heart_full','heart_empty'):
        im,draw=canvas();draw.polygon(heart,fill=PALETTE['o'])
        draw.line(heart+[heart[0]],fill=PALETTE['S'],width=3)
        if name=='heart_full':
            draw.polygon([(12,17),(17,12),(24,12),(32,20),(40,12),(47,12),(52,17),(52,30),(32,51),(12,30)],fill=PALETTE['E'])
            draw.line([(16,22),(20,17),(24,17)],fill=PALETTE['F'],width=5)
        images[name]=im
    for i,im in enumerate(flame_frames()):
        images[f'fire_{i}']=im;lit.add(f'fire_{i}')
    im,draw=canvas();draw.ellipse((3,3,60,60),outline=PALETTE['Y'],width=3)
    images['shock_ring']=im
    for name,im in iron_brick().items():
        images[name]=im;lit.add(name)


def flame_frames():
    """Eight frames of a small flame tongue for burning balls.

    Drawn on a 32-pixel grid and doubled, like the marbles' 2x2 blocks, so the
    runtime's 32-pixel quad samples it exactly. A main tongue with a rounded
    base sways and stretches while two side licks rise out of phase; in some
    frames a lick tears off above the tip. Heat sets the colour: a white core
    just above the base, then yellow, orange and a red edge."""
    frames=[]
    for i in range(8):
        phase=i*math.tau/8
        small=Image.new('RGBA',(32,32),PALETTE['.'])
        heat={}
        def tongue(cx,height,half,sway,bend,hot=1.0):
            for h in range(max(1,round(height))):
                t=h/height
                # Rounded base, then a taper to the tip.
                w=half*min(1.0,(h+1.5)/4.5)*(1-t)**0.75+0.35
                x0=cx+sway*math.sin(bend+t*2.4)*t
                core=1-abs(t-0.18)/0.82
                for x in range(32):
                    d=abs(x+0.5-x0)/w
                    if d<=1:
                        y=31-h
                        heat[(x,y)]=max(heat.get((x,y),0),hot*(core*0.75+(1-d)*0.5))
        tongue(16,22+3*math.sin(phase),6.6,2.2,phase)
        # The side licks stop short of white-hot.
        tongue(11.5,11+4*math.sin(phase+2.3),3.2,-1.6,phase+1.1,0.8)
        tongue(20.5,12+4*math.sin(phase+4.4),3.2,1.6,phase+2.7,0.8)
        if math.sin(2*phase+0.6)>0.35:
            tip=round(16+2.2*math.sin(phase+2.4)),round(31-22-3*math.sin(phase))-3
            for dx,dy in ((0,0),(0,-1),(1,0)):
                heat[(tip[0]+dx,tip[1]+dy)]=0.3
        for (x,y),v in heat.items():
            if 0<=x<32 and 0<=y<32:
                c='Z' if v>1.05 else 'J' if v>0.82 else 'K' if v>0.58 else 'L'
                small.putpixel((x,y),PALETTE[c])
        frames.append(small.resize((64,64),Image.NEAREST))
    return frames


def iron_brick():
    """The iron brick: 120 riveted pixels centred on a 128-pixel square, cut
    into four 64-pixel quarters along its plate seams.

    Four plates lit from the upper left, each shading from a lighter top to a
    darker foot through dithered steps; a heavy bevel, bright on the top and
    left, dark below and right; grooved seams; domed corner rivets with a
    glint; a few scratches and rust bleeding down from some rivets."""
    size=128;lo,hi=4,123
    im=Image.new('RGBA',(size,size),PALETTE['.'])
    put=lambda x,y,c:im.putpixel((x,y),PALETTE[c])
    rng=rng_for('iron_brick')
    ramp='Ω;;:'
    for plate_y in (lo,65):
        for y in range(plate_y,plate_y+59):
            v=(y-plate_y)/58*(len(ramp)-1)
            for x in range(lo,hi+1):
                # Ordered dither between neighbouring ramp steps.
                step=int(v)+(1 if (v%1)>((x*3+y*5)%8)/8 else 0)
                put(x,y,ramp[min(step,len(ramp)-1)])
    for _ in range(14):
        x,y=rng.randrange(lo+8,hi-14),rng.randrange(lo+8,hi-4)
        for k in range(rng.randrange(4,11)):put(x+k,y-k//3,'δ' if k%3 else 'ψ')
    # Plate seams: a dark groove with a lit lower/right lip.
    for k in range(lo,hi+1):
        for g in (62,63,64):put(g,k,'-');put(k,g,'-')
        put(65,k,'ψ');put(k,65,'ψ')
    # Bevel: bright top/left, dark bottom/right, a near-black edge around all.
    for k in range(lo,hi+1):
        for d,(light,dark) in enumerate((('φ','i'),('χ','b'),('ψ','Φ'),('Ω','B'))):
            if k>lo+d and k<hi-d:
                put(k,lo+1+d,light);put(lo+1+d,k,light)
                put(k,hi-1-d,dark);put(hi-1-d,k,dark)
        for edge in ((k,lo),(lo,k),(k,hi),(hi,k)):put(*edge,'o')
    for plate_x in (lo,65):
        for plate_y in (lo,65):
            for cx in (plate_x+10,plate_x+47):
                for cy in (plate_y+10,plate_y+47):
                    for dx in range(-3,3):
                        for dy in range(-3,3):
                            if dx*dx+dy*dy<=8:put(cx+dx,cy+dy,'Ω' if dx+dy<0 else ':')
                    for dx,dy in ((-1,-2),(-2,-1),(-1,-1)):put(cx+dx,cy+dy,'χ')
                    put(cx-1,cy-1,'φ');put(cx+1,cy+2,'-');put(cx+2,cy+1,'-')
                    # Rust bleeds down from some rivets.
                    if rng.random()<0.45:
                        x=cx+rng.choice((-1,0,1))
                        for k in range(3,3+rng.randrange(6,16)):
                            if cy+k<plate_y+56 and rng.random()<0.85:put(x,cy+k,rng.choice(('(','ω','"')))
    return {f'iron_brick_big_{row}_{col}':im.crop((col*64,row*64,col*64+64,row*64+64))
            for row in (0,1) for col in (0,1)}
