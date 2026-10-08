"""HUD hearts, flame frames, the shock ring, iron plates and their frost coating."""
import math

from PIL import Image, ImageDraw

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
    # Frost stays readable in the dark; the iron underneath still receives lighting.
    images.update(iron_frost())
    scraps,scrap_frost=iron_scraps()
    images.update(scraps);lit.update(scraps)
    images.update(scrap_frost)
    images['ice_crystal']=ice_crystal()
    images['ice_chip']=ice_chip()
    images['ice_ring']=ice_ring()


def iron_scraps():
    """Four fractured chunks at their 32-pixel runtime size, with matching ice.

    Chipped corners, torn silver edges, uneven fracture planes, rust and deep
    cracks distinguish loose scrap from the large block's riveted plates.
    Draw on a 32-pixel grid then double, keeping every runtime pixel crisp.
    """
    shapes=[[(4,1),(24,1),(30,7),(29,23),(24,30),(7,29),(1,21),(2,8)],
            [(2,5),(11,1),(27,2),(30,11),(27,19),(30,26),(20,30),(4,27),(1,18)],
            [(6,1),(23,3),(30,1),(29,24),(24,30),(12,27),(5,30),(1,22),(2,8)],
            [(2,2),(21,1),(29,7),(26,14),(30,23),(25,29),(5,30),(1,24),(4,17),(1,10)]]
    metal,ice={},{}
    for variant,shape in enumerate(shapes):
        mask=Image.new('L',(32,32),0)
        ImageDraw.Draw(mask).polygon(shape,fill=255)
        rng=rng_for(f'iron_scrap_{variant}')
        im=Image.new('RGBA',(32,32),PALETTE['.'])
        draw=ImageDraw.Draw(im)
        ramp='ψΩ;:Φ'
        for y in range(32):
            for x in range(32):
                if mask.getpixel((x,y)):
                    shade=min(4,int((x*.3+y*.7)/7))
                    if rng.random()<.08:shade=min(4,shade+1)
                    im.putpixel((x,y),PALETTE[ramp[shade]])
        # Two exposed fracture planes with jagged bright ridges and dark splits.
        ridge=[(4,10+variant),(11,7+variant),(16,11),(25,5+variant)]
        draw.polygon(ridge+[(28,12),(17,17),(9,15)],fill=PALETTE['Ω'])
        draw.line(ridge,fill=PALETTE['χ'],width=1)
        crack=[(8+variant,8),(13,15),(10+variant,21),(17,26)]
        draw.line(crack,fill=PALETTE['o'],width=2)
        draw.line([(x+1,y) for x,y in crack],fill=PALETTE['ψ'],width=1)
        draw.line([(13,15),(20,17),(25,14)],fill=PALETTE['o'])
        for _ in range(8):
            x,y=rng.randrange(4,28),rng.randrange(7,28)
            if mask.getpixel((x,y)):
                draw.point((x,y),fill=PALETTE[rng.choice(('ω','"','χ'))])
        draw.line(shape+[shape[0]],fill=PALETTE['o'],width=1)
        # Freshly torn upper/left edges catch the light; no regular bolt pattern.
        draw.line(shape[:3],fill=PALETTE['φ'],width=1)
        draw.line([shape[-1],shape[0]],fill=PALETTE['χ'],width=1)
        blank=Image.new('RGBA',(32,32),PALETTE['.'])
        im=Image.composite(im,blank,mask)
        metal[f'iron_scrap_{variant}']=im.resize((64,64),Image.NEAREST)

        frost=Image.new('RGBA',(32,32),PALETTE['.'])
        frozen=ImageDraw.Draw(frost)
        frozen.polygon(shape,fill=PALETTE['x'])
        frozen.polygon([(4,4),(25,5),(14,18)],fill=PALETTE['_'])
        frozen.polygon([(14,18),(28,12),(24,29)],fill=PALETTE['`'])
        frozen.line(shape+[shape[0]],fill=PALETTE['_'],width=1)
        frozen.line(shape[:3],fill=PALETTE['`'],width=2)
        frozen.line(crack,fill=PALETTE[','],width=1)
        for x,y in ((7,8),(22,22)):
            frozen.line((x-1,y,x+1,y),fill=PALETTE['Ψ'])
            frozen.line((x,y-1,x,y+1),fill=PALETTE['Ψ'])
        frost=Image.composite(frost,blank,mask)
        ice[f'iron_scrap_frost_{variant}']=frost.resize((64,64),Image.NEAREST)
    return metal,ice


def ice_chip():
    """A faceted sharp shard; fractures shed chips rather than intact snowflakes."""
    im,draw=canvas()
    draw.polygon([(35,8),(48,24),(36,55),(19,39),(23,20)],fill=PALETTE['χ'])
    draw.polygon([(35,8),(36,33),(23,20)],fill=PALETTE['Ψ'])
    draw.polygon([(35,8),(48,24),(36,33)],fill=PALETTE['φ'])
    draw.polygon([(36,33),(36,55),(19,39)],fill=PALETTE['φ'])
    draw.line([(35,8),(48,24),(36,55)],fill=PALETTE['Ψ'],width=2)
    return im


def ice_ring():
    """Broken white arcs keep an expanding cold pulse crisp against the night."""
    im,draw=canvas()
    for arc in range(6):
        draw.arc((5,5,58,58),arc*60+4,arc*60+53,fill=PALETTE['Ψ'],width=2)
        draw.arc((8,8,55,55),arc*60+10,arc*60+40,fill=PALETTE['χ'],width=1)
    return im


def ice_crystal():
    """Six white snowflake arms and forked tips, tinted blue by cold-particle configs."""
    im,draw=canvas()
    center=(32,32)
    for arm in range(6):
        angle=arm*math.tau/6
        point=lambda r,a:(round(32+r*math.cos(a)),round(32+r*math.sin(a)))
        draw.line([center,point(23,angle)],fill=PALETTE['φ'],width=3)
        draw.line([center,point(21,angle)],fill=PALETTE['Ψ'],width=1)
        for radius in (10,17):
            root=point(radius,angle)
            for side in (-1,1):
                end=(round(root[0]+7*math.cos(angle+side*math.pi/3)),
                     round(root[1]+7*math.sin(angle+side*math.pi/3)))
                draw.line([root,end],fill=PALETTE['Ψ'],width=2)
    draw.polygon([(32,27),(37,30),(37,34),(32,37),(27,34),(27,30)],fill=PALETTE['Ψ'])
    return im


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


def iron_frost():
    """A translucent ice shell with faceted faces, rime, branching cracks and icicles.

    Author the shell across the whole block before cutting at its plate seams,
    so the frost and fractures join cleanly between its four runtime quarters.
    All colours belong to the existing palette; there are no new materials."""
    im=Image.new('RGBA',(128,128),PALETTE['.'])
    draw=ImageDraw.Draw(im)
    rng=rng_for('iron_frost')
    # Transparent white glaze leaves the rivets and plates visible below it.
    draw.rectangle((4,4,123,123),fill=PALETTE['x'])
    facets=[([(7,12),(48,7),(17,59)],'_'),
            ([(48,7),(77,10),(17,59)],'`'),
            ([(81,8),(118,8),(119,58)],'_'),
            ([(10,72),(53,67),(9,116)],'`'),
            ([(70,72),(117,55),(117,116)],'_'),
            ([(72,76),(117,116),(60,116)],'`')]
    for points,c in facets:
        draw.polygon(points,fill=PALETTE[c])
    # Deep blue fracture shadows, with narrow bright ice on their lit side.
    cracks=[[(24,7),(33,32),(29,53),(46,76),(40,103),(49,122)],
            [(33,32),(52,24),(66,31)],[(46,76),(19,87),(7,82)],
            [(118,26),(97,43),(101,64),(82,87),(90,121)],
            [(101,64),(117,76)],[(82,87),(63,83),(55,91)]]
    for points in cracks:
        draw.line(points,fill=PALETTE[','],width=3)
        draw.line([(x-1,y) for x,y in points],fill=PALETTE['`'],width=1)
    # Thick, uneven hoarfrost on the rim and thinner frost on the plate grooves.
    for x in range(4,124):
        depth=5+round(2*math.sin(x*.21))+rng.randrange(3)
        draw.line((x,4,x,4+depth),fill=PALETTE['`'])
        draw.point((x,4),fill=PALETTE['Ψ'])
    draw.line([(4,12),(4,120),(122,123),(123,7)],fill=PALETTE['_'],width=3)
    draw.line([(5,12),(5,117)],fill=PALETTE['`'],width=2)
    draw.line([(8,63),(120,63)],fill=PALETTE['_'],width=2)
    draw.line([(63,12),(63,119)],fill=PALETTE['_'],width=2)
    for x in (17,39,71,97,115):
        length=rng.randrange(7,13)
        draw.polygon([(x-3,114),(x+3,114),(x,114+length)],fill=PALETTE['`'])
        draw.line((x-1,114,x,112+length),fill=PALETTE['Ψ'],width=1)
    # Crisp crystal glints on the ice rather than a uniform blue tint.
    for x,y in ((15,19),(53,47),(89,18),(112,53),(24,104),(76,98)):
        draw.line((x-3,y,x+3,y),fill=PALETTE['Ψ'])
        draw.line((x,y-4,x,y+4),fill=PALETTE['Ψ'])
    return {f'iron_frost_big_{row}_{col}':im.crop((col*64,row*64,col*64+64,row*64+64))
            for row in (0,1) for col in (0,1)}
