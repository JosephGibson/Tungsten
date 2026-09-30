"""HUD hearts, burning-ball flame frames and the explosion shock ring."""
import math

from art import PALETTE, canvas


def enrich_art(images,lit,animations):
    heart=[(8,15),(15,8),(25,8),(32,15),(39,8),(49,8),(56,15),(56,31),(32,56),(8,31)]
    for name in ('heart_full','heart_empty'):
        im,draw=canvas();draw.polygon(heart,fill=PALETTE['o'])
        draw.line(heart+[heart[0]],fill=PALETTE['S'],width=3)
        if name=='heart_full':
            draw.polygon([(12,17),(17,12),(24,12),(32,20),(40,12),(47,12),(52,17),(52,30),(32,51),(12,30)],fill=PALETTE['E'])
            draw.line([(16,22),(20,17),(24,17)],fill=PALETTE['F'],width=5)
        images[name]=im
    fire=[]
    for i in range(8):
        im,draw=canvas();phase=i*math.tau/8
        swing=round(math.sin(phase)*5)
        draw.polygon([(30,3),(38+swing,16),(35,22),(49,14),(46,32),(57,40),(53,53),(42,60),(23,61),(10,53),(8,41),(19,28),(17,18),(27,25),(33,17)],fill=PALETTE['r'])
        draw.polygon([(31+swing,13),(36,32),(43,26),(48,44),(43,54),(31,58),(18,52),(14,42),(28,30)],fill=PALETTE['A'])
        draw.polygon([(29,29),(34+swing,39),(38,37),(41,48),(33,55),(24,50),(23,43)],fill=PALETTE['y'])
        draw.ellipse((28,43,35,53),fill=PALETTE['Y']);fire.append(im);images[f'fire_{i}']=im;lit.add(f'fire_{i}')
    im,draw=canvas();draw.ellipse((3,3,60,60),outline=PALETTE['Y'],width=3)
    images['shock_ring']=im
