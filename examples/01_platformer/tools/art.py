"""Palette-only pixel art. Grids are source; derived poses keep hard edges."""
import hashlib
import json
import math
import random
from pathlib import Path

from PIL import Image, ImageDraw

TOOLS = Path(__file__).resolve().parent
def validate_palette(palette):
    if palette.get('.') != [0, 0, 0, 0]:
        raise ValueError('palette must define transparent dot')
    for symbol, rgba in palette.items():
        if len(symbol) != 1 or len(rgba) != 4 or any(type(c) is not int or not 0 <= c <= 255 for c in rgba):
            raise ValueError('palette requires one-character symbols and four RGBA bytes')
    return {k: tuple(v) for k, v in palette.items()}


PALETTE = validate_palette(json.loads((TOOLS / 'palette.json').read_text()))


def rng_for(name):
    return random.Random(int.from_bytes(hashlib.sha256(name.encode()).digest()[:8], 'big'))


def read_grid(text, width=64, height=64):
    rows = text.splitlines()
    if len(rows) != height or any(len(row) != width for row in rows):
        raise ValueError(f'grid must be exactly {width}x{height}')
    if any(c not in PALETTE for row in rows for c in row):
        raise ValueError('unknown palette symbol')
    im = Image.new('RGBA', (width, height))
    im.putdata([PALETTE[c] for row in rows for c in row])
    return im


def grid(name, size=(64, 64)):
    im = read_grid((TOOLS / 'grids' / f'{name}.grid').read_text(), *size)
    box = im.getbbox()
    if not box or box[0] < 1 or box[1] < 1 or box[2] >= size[0] or box[3] >= size[1]:
        raise ValueError(f'{name}: source grid must retain a transparent border')
    return im


def canvas(size=(64, 64), color='.'):
    im = Image.new('RGBA', size, PALETTE[color])
    return im, ImageDraw.Draw(im)


def shift_part(im, box, dx=0, dy=0):
    out = im.copy()
    out.paste(PALETTE['.'], box)
    part = im.crop(box)
    # Source art leaves enough space for each integer displacement.
    out.alpha_composite(part, (box[0] + dx, box[1] + dy))
    return out


def auxiliary(im):
    normal = Image.new('RGBA', im.size, (128, 128, 255, 0))
    emissive = Image.new('RGBA', im.size, (0, 0, 0, 255))
    glow = {PALETTE[c] for c in 'yYCc'}
    alpha = im.getchannel('A')
    for y in range(im.height):
        for x in range(im.width):
            rgba = im.getpixel((x, y))
            if not rgba[3]:
                continue
            # Same final geometry drives bevel normals and detail emission.
            left = alpha.getpixel((max(0, x-1), y)) / 255
            right = alpha.getpixel((min(im.width-1, x+1), y)) / 255
            up = alpha.getpixel((x, max(0, y-1))) / 255
            down = alpha.getpixel((x, min(im.height-1, y+1))) / 255
            nx = (left-right)*0.6 + (x/im.width-0.5)*0.25
            ny = (up-down)*0.6 + (y/im.height-0.5)*0.25
            length = math.sqrt(nx*nx+ny*ny+1)
            normal.putpixel((x,y), (round(128+127*nx/length), round(128+127*ny/length), round(128+127/length), rgba[3]))
            if rgba in glow:
                emissive.putpixel((x,y), (*rgba[:3],255))
    return normal, emissive


def terrain(name):
    im, d = canvas(color='i')
    rng = rng_for(name)
    for y in range(4,64,16):
        offset = 0 if (y//16)%2 else 16
        for x in range(-offset,64,32):
            d.rectangle((x+1,y+1,x+30,y+14), fill=PALETTE['b'])
            d.line((x+2,y+1,x+29,y+1), fill=PALETTE['s'])
            d.line((x+2,y+14,x+29,y+14), fill=PALETTE['o'])
    for _ in range(100):
        x,y = rng.randrange(3,58),rng.randrange(8,61)
        d.rectangle((x,y,x+rng.randrange(1,4),y), fill=PALETTE[rng.choice('ibs')])
    top = name != 'stone_wall' and not name.startswith('fill')
    if top:
        d.rectangle((0,0,63,2),fill=PALETTE['m'])
        d.rectangle((0,3,63,5),fill=PALETTE['g'])
        d.rectangle((0,6,63,7),fill=PALETTE['t'])
        for x in range(3,61,4):
            length=rng.randrange(2,8)
            d.line((x,5,x,5+length),fill=PALETTE['T'],width=2)
        # Identical perimeter on every variant: no mismatched top seams.
    if name in ('ground_left','corner_convex_left','ledge'):
        d.line((0,8,0,63),fill=PALETTE['S'],width=2)
    if name in ('ground_right','corner_convex_right','ledge'):
        d.line((62,8,62,63),fill=PALETTE['d'],width=2)
    if name.startswith('corner_concave'):
        x=2 if name.endswith('left') else 61
        d.line((x,8,x,63),fill=PALETTE['T'],width=3)
    if name.startswith('platform') or name in ('stone_platform','ledge','bridge'):
        d.rectangle((0,22,63,63),fill=PALETTE['.'])
        d.line((0,21,63,21),fill=PALETTE['o'],width=2)
        if name=='bridge':
            for x in range(0,64,16):
                d.rectangle((x+1,8,x+14,18),fill=PALETTE['a'])
                d.line((x+2,9,x+13,9),fill=PALETTE['A'])
    return im


def backdrop(name):
    if name=='sky':
        im,d=canvas((1024,512),'i')
        colors=[PALETTE[str(i)] for i in range(10)]
        noise=rng_for('sky-dither')
        for y in range(512):
            band=min(8,y*9//512); fraction=(y*9%512)/512
            for x in range(1024):
                # Adjacent ramp colors and seeded noise avoid a giant checkerboard
                # when the broad coverage quad is viewed at minimum zoom.
                im.putpixel((x,y),colors[band+int(fraction>noise.random())])
        rng=rng_for(name)
        for _ in range(140):
            x,y=rng.randrange(1024),rng.randrange(310)
            d.point((x,y),fill=PALETTE['S'])
        d.ellipse((766,58,804,96),fill=PALETTE['w'])
        d.ellipse((756,53,794,90),fill=PALETTE['i'])
        return im
    im,d=canvas((1024,256))
    # Periodic analytic profiles guarantee exact wrap at the strip boundary.
    for x in range(1024):
        if name=='distant_ridges':
            y=round(105+44*math.sin(2*math.pi*x/1024)+23*math.cos(6*math.pi*x/1024))
            d.line((x,y,x,255),fill=PALETTE['b'])
            y2=round(165+25*math.cos(4*math.pi*x/1024))
            d.line((x,y2,x,255),fill=PALETTE['d'])
        else:
            y=round(177+13*math.sin(8*math.pi*x/1024))
            d.line((x,y,x,255),fill=PALETTE['n'])
    if name=='near_woodland':
        for x in range(48,1000,73):
            y=60+int(35*math.sin(x))
            d.line((x,y,x+6,246),fill=PALETTE['n'],width=8)
            for k in range(4):
                yy=y+k*25
                d.polygon([(x,yy),(x-30-k*4,yy+45),(x+33+k*4,yy+40)],fill=PALETTE['t' if k==0 else 'n'])
    # Align both edge columns for visibly seamless repeated strips.
    im.paste(im.crop((0,0,1,256)),(1023,0))
    return im


def build_art():
    images={}; lit=set(); animations={}
    def clip(name, frames, duration, looping=True, lighting=False):
        names=[]
        for i,im in enumerate(frames):
            sprite = ('player' if i==0 else f'player_idle_{i}') if name=='player_idle' else ('ball' if i==0 else f'ball_{i}') if name=='ball_spin' else ('lantern' if i==0 else f'lantern_{i}') if name=='torch_flicker' else ('vines' if i==0 else f'vines_{i}') if name=='vines_sway' else f'{name}_{i}'
            images[sprite]=im; names.append(sprite)
            if lighting:lit.add(sprite)
        animations[name]={'looping':looping,'frames':[{'sprite':'ex10_'+s,'duration_ms':duration} for s in names]}
    idle=grid('player_idle')
    frames=[shift_part(idle,(15,5,56,55),dy=dy) for dy in (0,-1,0,0)]
    blink=ImageDraw.Draw(frames[3]);blink.rectangle((33,22,36,23),fill=PALETTE['o']);blink.line((33,23,36,23),fill=PALETTE['A'])
    clip('player_idle',frames,240,lighting=True)
    walk=[]
    for i in range(8):
        im=grid('player_contact' if i%4 in (0,3) else 'player_passing')
        if i>=4:
            # Exchange boot phases without mirroring the held lantern.
            legs=im.crop((12,54,46,61)).transpose(Image.Transpose.FLIP_LEFT_RIGHT)
            im.paste(legs,(12,54))
        if i%4==2:im=shift_part(im,(15,5,56,48),dy=-1)
        walk.append(im)
    clip('player_walk',walk,80,lighting=True)
    for name,count,looping,duration in [('jump',2,False,90),('fall',2,True,140),('land',3,False,60)]:
        base=grid('player_'+name)
        frames=[shift_part(base,(42,22,56,55),dx=(i%2)*-1) for i in range(count)]
        # Landing artwork changes folds/eyes only; runtime owns squash.
        if name=='land':frames[-1]=idle.copy()
        clip('player_'+name,frames,duration,looping,lighting=True)
    orb=grid('orb')
    frames=[]
    for i in range(6):
        im=orb.rotate(i*60,resample=Image.Resampling.NEAREST)
        # Palette cycle only on the runes; normals derive from the rotated mask.
        remap={PALETTE['c']:PALETTE['y' if i%2 else 'c'],PALETTE['C']:PALETTE['Y' if i%2 else 'C']}
        im.putdata([remap.get(px,px) for px in im.get_flattened_data()]);frames.append(im)
    clip('ball_spin',frames,90,lighting=True)
    lamp=grid('lantern'); frames=[]
    for i in range(6):
        im=lamp.copy();d=ImageDraw.Draw(im)
        d.polygon([(29,35),(28+i%3,29),(32,23+i%3),(36,31),(34,37)],fill=PALETTE['y'])
        d.line((32,28+i%3,32,34),fill=PALETTE['Y'],width=2);frames.append(im)
    clip('torch_flicker',frames,100,lighting=True)
    vine=grid('vines');clip('vines_sway',[shift_part(vine,(3,4,62,48),dx=x) for x in (0,1,0,-1)],200)
    images['marker']=grid('marker')
    gate=grid('gate',(192,192))
    for row in range(3):
        for col in range(3):images[f'gate_big_{row}_{col}']=gate.crop((col*64,row*64,(col+1)*64,(row+1)*64))
    for name in ['ground','ground_1','ground_2','ground_3','ground_left','ground_right','corner_convex_left','corner_convex_right','corner_concave_left','corner_concave_right','stone_wall','fill_1','fill_2','platform','platform_1','platform_2','stone_platform','ledge','bridge']:
        images[name]=terrain(name)
    for name in ['sky','distant_ridges','near_woodland']:images[name]=backdrop(name)
    frames=[]
    channels=rng_for('waterfall-channels')
    offsets=[channels.randrange(32) for _ in range(15)]
    for i in range(4):
        im,d=canvas()
        for x in range(10,55):
            for y in range(64):
                # A 32-pixel period wraps every segment. Independent channel
                # offsets make vertical water streaks rather than diagonal bars.
                phase=(y-i*8-offsets[(x-10)//3])%32
                tone='d' if x%9<3 else 'D'
                if phase<10 and x%3!=0: tone='c'
                if phase<4 and x%3==1: tone='C'
                im.putpixel((x,y),PALETTE[tone])
        frames.append(im)
    clip('waterfall_flow',frames,100)
    for name in ['dust','spark','droplet','mote','cursor']:
        im,d=canvas()
        if name=='dust':
            for box in [(14,29,37,45),(25,18,48,40),(32,32,54,49)]:d.ellipse(box,fill=PALETTE['w'])
            d.rectangle((24,29,43,40),fill=PALETTE['S'])
        elif name=='droplet':d.polygon([(32,12),(41,35),(37,46),(29,47),(24,36)],fill=PALETTE['C'])
        elif name=='cursor':
            for pts in [[(14,24),(14,14),(24,14)],[(40,14),(50,14),(50,24)],[(14,40),(14,50),(24,50)],[(40,50),(50,50),(50,40)]]:d.line(pts,fill=PALETTE['Y'],width=3)
            d.rectangle((30,30,33,33),fill=PALETTE['Y'])
        else:
            d.polygon([(32,12),(36,27),(50,32),(36,36),(32,51),(28,36),(14,32),(28,28)],fill=PALETTE['Y' if name=='spark' else 'C'])
        images[name]=im
    from polish_art import enrich_art
    enrich_art(images,lit,animations)
    return images,lit,animations


def validate_art(images,lit):
    allowed=set(PALETTE.values())
    for name,im in images.items():
        if name not in ('sky','distant_ridges','near_woodland','clouds_far','clouds_near') and im.size!=(64,64):raise ValueError(f'{name}: wrong dimensions')
        if not set(im.get_flattened_data())<=allowed:raise ValueError(f'{name}: outside palette')
        if name.startswith(('player', 'ball', 'lantern', 'vines')) or name == 'marker':
            box=im.getbbox()
            if not box or box[0]<1 or box[1]<1 or box[2]>63 or box[3]>63:raise ValueError(f'{name}: clipped frame')
            if name.startswith('player') and box[3]!=61:raise ValueError(f'{name}: unstable foot anchor')
    for name in ('ground_1','ground_2','ground_3'):
        if images[name].crop((0,0,64,6)).tobytes()!=images['ground'].crop((0,0,64,6)).tobytes():raise ValueError('terrain seam')
    for name in lit:
        n,e=auxiliary(images[name])
        if n.size!=images[name].size or e.size!=images[name].size:raise ValueError('auxiliary size')
        if n.getchannel('A').tobytes()!=images[name].getchannel('A').tobytes():raise ValueError('normal mask mismatch')
