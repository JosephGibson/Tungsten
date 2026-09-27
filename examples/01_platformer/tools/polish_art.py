"""Additional native pixel art for hazards, ruin scenery and atmosphere."""
import math
from PIL import Image, ImageDraw, ImageChops
from art import PALETTE, canvas, grid, rng_for, shift_part

BACKDROPS = ('sky', 'distant_ridges', 'near_woodland', 'clouds_far', 'clouds_near')


def walk_frames():
    # One full stride: planted foot travels backward while the other clears the
    # ground through heel lift, passing, extension and heel strike. No boot flip.
    feet = [(20,60),(18,60),(20,60),(24,60),(29,58),(34,55),
            (39,54),(43,56),(43,59),(38,60),(32,60),(26,60)]
    frames = []
    body = grid('player_idle')
    body.paste(PALETTE['.'], (10,53,45,64))
    for i in range(12):
        im, draw = canvas()
        for hip, foot, color in [(27,feet[(i+6)%12],'b'),(34,feet[i],'s')]:
            x,y = foot
            knee = (round((hip+x)/2)+1, 53 if y==60 else 50)
            draw.line([(hip,48),knee,(x,y-2)],fill=PALETTE['o'],width=7)
            draw.line([(hip,48),knee,(x,y-3)],fill=PALETTE[color],width=3)
            draw.rectangle((x-3,y-2,x+4,y),fill=PALETTE['o'])
            draw.line((x-2,y-2,x+3,y-2),fill=PALETTE['S'])
        bob = [0,0,-1,-2,-1,0][i%6]
        torso = shift_part(body,(12,4,57,56),dy=bob)
        torso = shift_part(torso,(44,36+bob,57,56),dx=[0,1,1,0,-1,-1][i%6])
        im.alpha_composite(torso)
        im.paste(PALETTE['.'], (0,61,64,64))
        frames.append(im)
    return frames


def sky():
    im,draw=canvas((1024,512),'0');noise=rng_for('polished-sky')
    for y in range(512):
        ramp = 8.9*(y/511)**1.3
        band=int(ramp)
        for x in range(1024):
            im.putpixel((x,y),PALETTE[str(min(9,band+int(noise.random()<ramp-band)))])
    for _ in range(65):
        x,y=noise.randrange(1024),noise.randrange(20,290)
        draw.point((x,y),fill=PALETTE['B' if y>150 else 'S'])
    return im


def clouds(name):
    im,draw=canvas((1024,256));rng=rng_for(name)
    for anchor in (110,425,770):
        y=rng.randrange(35,120)
        width=rng.randrange(125,235)
        for tone,dy in [('h',12),('H',6),('f',0)]:
            for offset in range(-width//2,width//2,17):
                height=round(13+20*math.sin(math.pi*(offset+width/2)/width))
                draw.ellipse((anchor+offset-28,y-height+dy,anchor+offset+38,y+height//2+dy),fill=PALETTE[tone])
        draw.line((anchor-width//2-10,y+17,anchor+width//2+24,y+17),fill=PALETTE['h'],width=4)
    return im


def landscape(name):
    im,draw=canvas((1024,256));rng=rng_for(name)
    if name=='distant_ridges':
        for layer,tone in [(0,'b'),(1,'d'),(2,'t')]:
            points=[]
            for x in range(-64,1089,32):
                y=90+layer*49+rng.randrange(-36,29)
                points.append((x,y))
            draw.polygon(points+[(1088,256),(-64,256)],fill=PALETTE[tone])
            if layer==0:
                for a,b in zip(points,points[1:]):
                    if a[1]<b[1]:draw.line([a,b],fill=PALETTE['B'],width=2)
        # Distant broken towers read as a ruin silhouette, not repeated blocks.
        for x in [178,555,812]:
            draw.rectangle((x,147,x+27,208),fill=PALETTE['d'])
            draw.polygon([(x-4,148),(x+5,133),(x+14,143),(x+27,136),(x+30,148)],fill=PALETTE['d'])
    else:
        draw.rectangle((0,211,1023,255),fill=PALETTE['n'])
        for x in range(-30,1100,109):
            x+=rng.randrange(-24,24);y=rng.randrange(65,145)
            draw.line((x,y,x+8,244),fill=PALETTE['n'],width=9)
            for dx,dy,r in [(-20,26,25),(22,31,34),(0,0,30),(-34,44,29)]:
                draw.ellipse((x+dx-r,y+dy-r,x+dx+r,y+dy+r),fill=PALETTE['t' if dy==0 else 'n'])
            draw.line([(x+5,224),(x-17,173),(x-35,164)],fill=PALETTE['n'],width=5)
    im.paste(im.crop((0,0,1,256)),(1023,0))
    return im


def tile_variants(images):
    for i in range(8):
        im,draw=canvas(color='i');rng=rng_for(f'rock-{i}')
        # Irregular stone silhouettes with broad planes; edge pixels remain
        # subdued so adjacent variants do not reveal an outlined 64px grid.
        for cy in range(-8,80,22):
            for cx in range(-12,90,29):
                x=cx+rng.randrange(-4,5);y=cy+rng.randrange(-3,4)
                pts=[(x,y+7),(x+7,y),(x+25,y+2),(x+30,y+17),(x+22,y+21),(x+3,y+19)]
                draw.polygon(pts,fill=PALETTE['b' if i%3 else 'd'])
                draw.line(pts[:3],fill=PALETTE['s' if i%4==0 else 'B'])
        for _ in range(18):
            x,y=rng.randrange(4,58),rng.randrange(5,59)
            draw.line((x,y,x+3,y),fill=PALETTE['b'])
        images[f'rock_{i}']=im
        moss=im.copy();d=ImageDraw.Draw(moss)
        d.rectangle((0,0,63,2),fill=PALETTE['m']);d.rectangle((0,3,63,5),fill=PALETTE['g'])
        for x in range(1,64,5):
            length=rng.randrange(4,19);d.line((x,5,x,5+length),fill=PALETTE['T'],width=3)
            if length>10:d.point((x,5+length),fill=PALETTE['G'])
        images[f'moss_rock_{i}']=moss
    for name in ['bridge_weathered','bridge_rope','slab_carved','slab_broken','lift_deck']:
        im=images['bridge' if name.startswith('bridge') or name=='lift_deck' else 'stone_platform'].copy();d=ImageDraw.Draw(im)
        if name in ('slab_carved','lift_deck'):
            for x in range(8,64,18):d.line([(x,10),(x+5,7),(x+10,10),(x+5,15),(x,10)],fill=PALETTE['c'])
        if name=='slab_broken':
            for x in (15,45):d.line([(x,7),(x+3,12),(x-2,20)],fill=PALETTE['i'],width=2)
        if name=='bridge_rope':
            d.line((0,17,63,17),fill=PALETTE['A']);d.line((0,19,63,19),fill=PALETTE['a'])
        if name=='bridge_weathered':
            for x in (5,24,48):d.line([(x,10),(x+5,12),(x+3,17)],fill=PALETTE['i'])
        images[name]=im


def scenery(images):
    # Composed pixel canvases become ordinary 64px pieces in the catalog.
    for name,size in [('oak',(192,192)),('arch',(256,192)),('roots',(192,128))]:
        im,draw=canvas(size);rng=rng_for(name)
        if name=='oak':
            draw.polygon([(70,187),(92,104),(88,51),(106,53),(112,121),(139,187)],fill=PALETTE['o'])
            draw.line([(91,172),(101,113),(98,70)],fill=PALETTE['a'],width=7)
            for branch in [[(101,119),(64,78),(29,67)],[(107,102),(141,63),(169,65)]]:draw.line(branch,fill=PALETTE['o'],width=10)
            for x,y,r in [(44,55,31),(72,33,32),(124,32,28),(154,60,29),(99,61,36)]:
                draw.ellipse((x-r,y-r+4,x+r,y+r),fill=PALETTE['t'])
                draw.arc((x-r+5,y-r+7,x+r-7,y+r-12),190,305,fill=PALETTE['T'],width=5)
            for _ in range(95):
                x,y=rng.randrange(18,174),rng.randrange(8,80)
                if im.getpixel((x,y))==PALETTE['t']:draw.line((x,y,x+3,y),fill=PALETTE['g'])
        elif name=='arch':
            for x in (21,191):
                draw.polygon([(x,190),(x+4,70),(x+37,68),(x+43,190)],fill=PALETTE['d'])
                draw.line((x+9,78,x+9,185),fill=PALETTE['b'],width=5)
            draw.arc((22,5,234,176),180,354,fill=PALETTE['d'],width=27)
            draw.arc((26,9,230,172),185,347,fill=PALETTE['b'],width=3)
            for x,y in [(47,51),(82,22),(159,23),(201,55)]:draw.line((x,y,x+11,y+13),fill=PALETTE['i'],width=3)
            draw.polygon([(112,3),(144,3),(139,33),(128,21),(119,37)],fill=PALETTE['.'])
        else:
            for x in (22,68,110,162):
                pts=[(x,1),(x-7,32),(x+10,57),(x-11,91),(x-18,117)]
                draw.line(pts,fill=PALETTE['t'],width=5);draw.line(pts,fill=PALETTE['T'],width=2)
                for xx,yy in pts[1:-1]:draw.polygon([(xx,yy),(xx+14,yy-6),(xx+10,yy+6)],fill=PALETTE['g'])
        for row in range(size[1]//64):
            for col in range(size[0]//64):images[f'{name}_big_{row}_{col}']=im.crop((col*64,row*64,(col+1)*64,(row+1)*64))
    for name in ['fern','flowers','mushrooms','rubble','grass','crystal','hanging_moss']:
        im,draw=canvas();rng=rng_for(name)
        if name=='rubble':
            for x,y,r in [(12,54,10),(27,50,13),(46,54,11)]:
                draw.polygon([(x-r,y+7),(x-r+2,y-3),(x,y-r),(x+r,y-1),(x+r,y+7)],fill=PALETTE['b']);draw.line([(x-r+2,y-3),(x,y-r),(x+r,y-1)],fill=PALETTE['s'])
        elif name=='crystal':
            for x,y in [(22,60),(38,60),(47,60)]:
                draw.polygon([(x-5,y),(x-6,y-20),(x,y-34),(x+7,y-19),(x+4,y)],fill=PALETTE['D']);draw.line([(x,y-30),(x+2,y-5)],fill=PALETTE['C'],width=2)
        else:
            for x in range(7,59,7):
                height=rng.randrange(10,32);y=61
                if name=='hanging_moss':y=4;height=-height
                draw.line([(x,y),(x-3,y-height)],fill=PALETTE['T'],width=2)
                if name=='flowers':draw.ellipse((x-5,y-height-4,x+1,y-height+2),fill=PALETTE['R' if x%2 else 'y'])
                elif name=='mushrooms':draw.pieslice((x-7,y-height-4,x+4,y-height+8),180,360,fill=PALETTE['A'])
                else:
                    for step in range(4,abs(height),6):
                        yy=y-step if height>0 else y+step
                        draw.line([(x-7,yy-5),(x,yy),(x+5,yy-3)],fill=PALETTE['g' if name=='fern' else 'T'],width=2)
        images[name]=im


def enrich_art(images,lit,animations):
    frames=walk_frames()
    for i,im in enumerate(frames):images[f'player_walk_{i}']=im;lit.add(f'player_walk_{i}')
    animations['player_walk']={'looping':True,'frames':[{'sprite':f'ex10_player_walk_{i}','duration_ms':55} for i in range(12)]}
    # A connected timber trestle reads differently from repeated masonry arches.
    im,draw=canvas()
    draw.polygon([(25,0),(39,0),(40,63),(24,63)],fill=PALETTE['a'])
    draw.line((28,0,28,63),fill=PALETTE['A'],width=2)
    draw.line((0,0,31,31,63,0),fill=PALETTE['o'],width=7)
    draw.line((0,0,31,31,63,0),fill=PALETTE['a'],width=4)
    images['timber_support']=im
    cliff=images['stone_wall'].copy()
    cliff.putdata([PALETTE['n'] if px==PALETTE['o'] else PALETTE['d'] for px in cliff.get_flattened_data()])
    images['cliff_back']=cliff
    im,draw=canvas()
    draw.ellipse((7,7,56,56),fill=PALETTE['w'])
    draw.ellipse((10,8,55,53),fill=PALETTE['I'])
    for box in [(17,19,25,26),(35,13,40,17),(39,33,49,43),(22,40,27,45)]:
        draw.ellipse(box,fill=PALETTE['w'])
    draw.arc((8,8,55,55),195,310,fill=PALETTE['Y'],width=2)
    images['moon']=im
    heart=[(8,15),(15,8),(25,8),(32,15),(39,8),(49,8),(56,15),(56,31),(32,56),(8,31)]
    for name in ('heart_full','heart_empty'):
        im,draw=canvas();draw.polygon(heart,fill=PALETTE['o'])
        draw.line(heart+[heart[0]],fill=PALETTE['S'],width=3)
        if name=='heart_full':
            draw.polygon([(12,17),(17,12),(24,12),(32,20),(40,12),(47,12),(52,17),(52,30),(32,51),(12,30)],fill=PALETTE['E'])
            draw.line([(16,22),(20,17),(24,17)],fill=PALETTE['F'],width=5)
        images[name]=im
    images['sky']=sky()
    for name in ('clouds_far','clouds_near'):images[name]=clouds(name)
    for name in ('distant_ridges','near_woodland'):images[name]=landscape(name)
    tile_variants(images);scenery(images)
    for name in list(images):
        if name.startswith(('rock_','moss_rock_','slab_','bridge','ground')):lit.add(name)
    im,draw=canvas()
    for x in (12,31,50):
        draw.polygon([(x-9,61),(x,33),(x+10,61)],fill=PALETTE['o'])
        draw.polygon([(x-6,58),(x,36),(x+2,58)],fill=PALETTE['S'])
        draw.line((x,37,x,49),fill=PALETTE['w'])
    draw.rectangle((3,60,60,62),fill=PALETTE['a']);images['spikes']=im
    fire=[]
    for i in range(8):
        im,draw=canvas();phase=i*math.tau/8
        swing=round(math.sin(phase)*5)
        draw.polygon([(30,3),(38+swing,16),(35,22),(49,14),(46,32),(57,40),(53,53),(42,60),(23,61),(10,53),(8,41),(19,28),(17,18),(27,25),(33,17)],fill=PALETTE['r'])
        draw.polygon([(31+swing,13),(36,32),(43,26),(48,44),(43,54),(31,58),(18,52),(14,42),(28,30)],fill=PALETTE['A'])
        draw.polygon([(29,29),(34+swing,39),(38,37),(41,48),(33,55),(24,50),(23,43)],fill=PALETTE['y'])
        draw.ellipse((28,43,35,53),fill=PALETTE['Y']);fire.append(im);images[f'fire_{i}']=im;lit.add(f'fire_{i}')
    animations['fire_dance']={'looping':True,'frames':[{'sprite':f'ex10_fire_{i}','duration_ms':70} for i in range(8)]}
    for name in ('vortex','vortex_core','shock_ring','halo','flame_glow'):
        im,draw=canvas()
        if name=='vortex':
            for arm in range(3):
                pts=[]
                for step in range(100):
                    t=step/99;angle=arm*math.tau/3+t*5.5;r=5+25*t
                    pts.append((round(31.5+math.cos(angle)*r),round(31.5+math.sin(angle)*r)))
                draw.line(pts,fill=PALETTE['v'],width=5);draw.line(pts,fill=PALETTE['c'],width=2)
                draw.line(pts[40:90],fill=PALETTE['C'])
        elif name=='vortex_core':
            draw.ellipse((12,12,51,51),fill=PALETTE['V']);draw.ellipse((16,16,47,47),fill=PALETTE['o']);draw.arc((14,14,49,49),200,320,fill=PALETTE['C'],width=2)
        elif name=='shock_ring':draw.ellipse((3,3,60,60),outline=PALETTE['Y'],width=3)
        else:
            for index,symbol in enumerate('ejklpqux' if name=='flame_glow' else '!@#$%^&*'):
                inset=index*3;draw.ellipse((inset,inset,63-inset,63-inset),fill=PALETTE[symbol])
        images[name]=im
