#!/usr/bin/env python3
"""Aceitação da cadeia do bedrock, etapa 1, no motor ntm-visual
(docs/maquinas.md, "Cadeia do bedrock, etapa 1").

Uso: python3 tools/test-game/bedrock.py <pasta do jogo> [etapa...]
O jogo aberto num mundo criativo pelo tools/test-game/run-visual.sh (Xvfb :15,
mudo), com os mods de tools/build-visual.sh. Etapas:
  escavar     a escavadora posta sobre um depósito de nível 1 (sem fluido;
              actinídeos 2,0, cristalinos 1,0 em (2920, −1886)), com a
              bateria criativa e a broca ferro: cava as 5 camadas sob ela
              (o resto do poço aberto antes, para o teste não levar horas)
              e tira 2 brutos;
  slop        o ore slopper com a bateria criativa e água de um barril na
              sua borda: os 2 brutos viram minérios BASE;
  centrifugar a centrífuga ao lado de uma bateria cheia: BASE de actinídeos
              → PRIMARY → fragmentos de urânio e tório;
  craftar     9 fragmentos de urânio numa bancada: 1 pó de urânio.
O depósito não guarda nada (tudo vem da posição): o teste o põe com fill.
Etapa 2 (cristalizador), no mesmo mundo:
  escavar3    uma segunda escavadora sobre um depósito de nível 3 em
              (2920, −1848) (actinídeos e cristalinos 2,0), com broca desh e
              ácido sulfúrico de um barril na camada de portas: 2 brutos;
  pilha       a pilha de brutos sai pelo shift-clique com o custom_data
              (a correção do motor para o criativo);
  lavar       2 BASE de actinídeos lavados num cristalizador com água; na
              centrífuga, 2 lavados dão 4 PRIMARY, contra 2 BASE → 2 PRIMARY;
  acido       PRIMARY + ácido sulfúrico num segundo cristalizador → PRIMARY
              sulfúrico → centrífuga (sem-sulfúrico + subproduto) → os
              fragmentos; 4 subprodutos lavados → os fragmentos dele.
Etapa 3 (forno pirolítico e eletrólise de fluidos), no mesmo mundo:
  torrar      40 BASE de actinídeos no forno: 40 torrados e 2.000 mB de
              vitríolo, que vai a um barril na porta dele; os torrados na
              centrífuga dão PRIMARY como os BASE;
  eletrolisar o vitríolo (levado em tanques de 1.000 mB) no eletrolisador:
              ácido sulfúrico e cloro a barris nas portas dele;
  ciclo       o ácido sulfúrico do eletrolisador num cristalizador novo (sem
              outro ácido): PRIMARY → PRIMARY sulfúrico.
Etapa 4 (forno a arco grande e linha de metal do eletrolisador):
  hidrogenar  PRIMARY de actinídeos + hidrogênio num cristalizador → PRIMARY
              FIRST;
  metal       o eletrolisador da etapa 3 (salvo como a linha de fluidos só:
              carrega como o de agora) com ácido nítrico de um barril e
              moldes de lingote a 6 blocos de cada ponta: 1 PRIMARY FIRST →
              urânio à esquerda, tório à direita, lingotes nos moldes;
  arco        o forno a arco em modo fluido com 3 eletrodos de grafite e um
              molde sob a bica: 2 PRIMARY FIRST → urânio fundido → lingotes.
A escavadora comum (anéis e interruptores), num chão de pedra novo:
  aneis       broca ferro, upgrade de efeito 1, britador e veio ligados: do
              2º nível em diante o poço é 7×7; uma veia de 3 minérios de
              ferro que sai do poço some inteira; a pedra vem como cascalho e
              o ferro como pó (o britador);
  seda        broca de aço diamantado com toque de seda: a pedra vem como
              pedra, não pedregulho;
  paredes     paredes ligadas, com água e um bolsão de ar na borda do poço
              na 3ª camada: os dois viram barricada.
Petróleo, lote A, no mesmo chão:
  petroleo    um liquefator e um solidificador lado a lado (as portas dos
              lados se tocam): 4 carvões viram 1.000 mB de óleo de carvão,
              que passa ao solidificador e volta sólido, 5 alcatrões de
              carvão.
Petróleo, lote B, no mesmo chão:
  petroleo_b  uma refinaria com óleo quente de um barril: as 4 frações e
              enxofre no slot 11; duas torres de fração empilhadas com óleo
              pesado: betume e smear descem à de baixo; um misturador com a
              receita do refrigerante (escolhida na lista), água de um
              barril e nitro: refrigerante.
Petróleo, lote C, num chão novo a leste:
  petroleo_c  uma caldeira sobre um firebox com carvão ferve a água de um
              barril em vapor; uma coqueificadora sobre outro firebox faz
              coque de óleo pesado; o craqueador com óleo e vapor de barris;
              o reformador (bateria e catalisador) com óleo leve dá
              aromáticos e reformgas; o hidrotratador com óleo e hidrogênio;
              o destilador a vácuo com óleo.
Etapa 5 do bedrock (solvente e radiossolvente):
  etapa5      aromáticos e reformgas recolhidos do reformador e nafta da
              refinaria em barris tipados; levados em tanques a dois
              misturadores (solvente; radiossolvente, com cloro): os dois
              solventes; cada um num cristalizador faz PRIMARY de actinídeos
              virar PRIMARY dissolvido / limpo; a centrífuga os separa;
  nivel4      uma escavadora sobre um depósito de nível 4 (chunk 164,−117),
              com 2.000 mB de solvente por extração: um bruto.
Petróleo, lote D, no chão do C:
  petroleo_d  um poço, um pumpjack e uma torre de fraturamento (bateria,
              overdrive 3) sobre depósitos postos com fill: o cano desce, o
              petróleo e o gás enchem os tanques; o poço entrega o óleo a um
              barril; a torre tira petróleo do bedrock com a solução de um
              barril.
Energia, lote 1, num chão novo a leste do C:
  energia_1   um queimador de óleo (tanques de óleo de aquecimento no slot de
              entrada, ligado, a taxa no máximo do controle), um aquecedor
              elétrico (bateria no plugue, potência 4 pela tela antiga) e um
              trocador de calor (líquido de arrefecimento quente de um
              barril), cada um com uma caldeira em cima e água de um barril:
              vapor nas três.
Grava <pasta>/bedrock-evidence.json e capturas bedrock-*.png.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
steps = [a for a in sys.argv[2:] if not a.startswith('sem-')] or ['escavar', 'slop', 'centrifugar', 'craftar']
CTL = ['/home/sacer/Workspace/horizon-visual/target/release/horizon-ctl', '--socket', str(game / 'control.sock')]
ev_path = game / 'bedrock-evidence.json'
evidence = json.loads(ev_path.read_text()) if ev_path.exists() else {}
# The deposit's chunk: x 2912..2927, z −1888..−1873. The excavator's core
# lands 3 cells off the click, so a click at z −1876 keeps it in there.
X, Z = 2920, -1876


def ctl(*args, check=True):
    out = subprocess.run(CTL + [str(a) for a in args], capture_output=True, text=True, timeout=300).stdout.strip()
    try:
        j = json.loads(out.splitlines()[-1]) if out else {}
    except ValueError:
        j = {'raw': out}
    if check and isinstance(j, dict) and j.get('error'):
        raise SystemExit(f'{args}: {j["error"]}')
    return j


def tp(x, y, z):
    """The vanilla command: horizon-ctl's tp answers ok here and doesn't
    always move the player."""
    ctl('command', f'tp @s {x} {y} {z}', check=False)
    time.sleep(0.3)


def screen():
    return ctl('screen')


def machine_slots():
    return [(s['slot'], s['item'], s['count'], s.get('components')) for s in screen().get('slots', []) if s.get('item') and s['slot'] >= 300]


def close():
    ctl('close', check=False)
    for _ in range(4):
        time.sleep(0.8)
        if ctl('screen').get('screen') not in ('mod', 'container'):
            return
        ctl('key', 'esc', 'tap')


def click_label(start, wait=2.0):
    for w in ctl('ui').get('widgets', []):
        if (w.get('label') or '').startswith(start):
            ctl('click', w['id'])
            time.sleep(wait)
            return
    raise SystemExit(f'sem "{start}": {[w.get("label") for w in ctl("ui").get("widgets", [])]}')


def give(item, n):
    ctl('give', item, n)
    for _ in range(10):
        time.sleep(0.5)
        if ctl('inventory').get('counts', {}).get(item, 0) >= n:
            return


def hot(item):
    return [s['slot'] for s in screen()['slots'] if s.get('item') == item and s['slot'] < 36]


def put(item, n, slot):
    give(item, n)
    ctl('click-slot', hot(item)[0])
    time.sleep(0.3)
    ctl('click-slot', slot)
    time.sleep(0.5)


def floor_y():
    p = game / 'bedrock-y'
    if p.exists():
        return int(p.read_text())
    tp(X + 0.5, 150, Z + 10.5)
    time.sleep(4)
    ctl('wait-ready', 120, check=False)
    y = int(ctl('state')['player']['position'][1])
    p.write_text(str(y))
    return y


def find(block, x0, y0, z0, x1, y1, z1):
    for b in ctl('blocks', x0, y0, z0, x1, y1, z1).get('blocks', []):
        if b[3].startswith(block + '['):
            return b[:3]
    raise SystemExit(f'{block} não achado')


def place(x, y, z, item, stand):
    close()
    tp(stand[0] + 0.5, stand[1], stand[2] + 0.5)
    give(item, 1)
    # A multiblock fills the clicked cell with its own blocks: ctl may call
    # that "did not change"; the caller finds the core.
    ctl('place', x, y, z, '--item', item, check=False)
    time.sleep(2)
    close()


def open_at(x, y, z, stand):
    close()
    tp(stand[0] + 0.5, stand[1], stand[2] + 0.5)
    ctl('interact', x, y, z)
    time.sleep(1.5)


def battery(x, y, z, stand):
    """A full battery in output mode (old screen; +10% only in creative)."""
    place(x, y, z, 'ntm:machine_battery', stand)
    ctl('look-at-block', x, y, z)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(2.5)
    for _ in range(12):
        click_label('Teste: +10%', 1.0)
    click_label('Trocar modo', 1.0)
    close()


def escavar():
    y = floor_y()
    if not (game / 'bedrock-chao').exists():
        ctl('fill', X - 6, y, Z - 9, X + 6, y + 8, Z + 12, 'minecraft:air')
        ctl('fill', X - 6, y - 1, Z - 9, X + 6, y - 1, Z + 12, 'minecraft:stone')
        (game / 'bedrock-chao').write_text('1')
    ctl('command', 'clear @s', check=False)
    if not any('excavator' in b[3] for b in ctl('blocks', X - 4, y, Z - 6, X + 4, y + 6, Z + 1).get('blocks', [])):
        place(X, y, Z, 'ntm:machine_excavator', (X, y, Z + 3))
    (cx, cy, cz) = find('ntm:machine_excavator', X - 4, y, Z - 4, X + 4, y + 6, Z + 4)
    # The deposit under the core, and the shaft open below its first 5 layers.
    ctl('fill', cx - 1, -63, cz - 1, cx + 1, cy - 9, cz + 1, 'minecraft:air')
    ctl('fill', cx, -64, cz, cx, -64, cz, 'ntm:ore_bedrock')
    # The legs stand on the floor: the screen through a body cell in reach.
    stand = (cx, y, cz + 6)
    open_at(cx, cy, cz + 3, stand)
    put('ntm:battery_creative', 1, 300)
    put('ntm:drillbit__ferro', 1, 304)
    ctl('click-button', 'Broca')
    time.sleep(2)
    ctl('screenshot', game / 'bedrock-escavadora.png')
    t0, raw = time.time(), []
    while time.time() - t0 < 600:
        time.sleep(10)
        open_at(cx, cy, cz + 3, stand)
        raw = [s for s in machine_slots() if s[1] == 'ntm:bedrock_ore_base']
        if sum(s[2] for s in raw) >= 2:
            break
    buf = machine_slots()
    for s in raw:
        ctl('click-slot', s[0], 'shift')
        time.sleep(0.4)
    close()
    inv = ctl('inventory')
    evidence['escavar'] = {'nucleo': [cx, cy, cz], 'buffer': buf, 'segundos': round(time.time() - t0, 1),
                           'brutos': [s for s in inv.get('hotbar', []) + inv.get('main', []) if s and s.get('item') == 'ntm:bedrock_ore_base'],
                           'poco': [ctl('block', cx, yy, cz).get('state') for yy in range(cy - 4, cy - 9, -1)]}
    print('escavar', evidence['escavar'])


def slop():
    y = floor_y()
    # On the stone floor south of the excavator.
    sx, sz = X, Z + 8
    if not any('slopper' in b[3] for b in ctl('blocks', sx - 4, y, sz - 4, sx + 4, y + 4, sz + 4).get('blocks', [])):
        place(sx, y, sz, 'ntm:machine_ore_slopper', (sx, y, sz + 4))
    (ox, oy, oz) = find('ntm:machine_ore_slopper', sx - 4, y, sz - 4, sx + 4, y + 4, sz + 4)
    # Water: a steel barrel just past the bottom layer's east edge.
    cells = [b for b in ctl('blocks', ox - 4, y, oz - 4, ox + 4, y, oz + 4).get('blocks', []) if 'dummy' in b[3] or 'slopper' in b[3]]
    bx = max(c[0] for c in cells) + 1
    if not ctl('block', bx, y, oz).get('state', '').startswith('ntm:barrel_steel'):
        place(bx, y, oz, 'ntm:barrel_steel', (bx + 2, y, oz))
    give('ntm:fluid_barrel_full__water', 1)
    ctl('select', 'ntm:fluid_barrel_full__water')
    close()
    tp(bx + 2.5, y, oz + 0.5)
    ctl('look-at-block', bx, y, oz)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(2.5)
    click_label('Despejar')
    # Input → Buffer → Output: the barrel gives its water to the slopper.
    while 'Saída' not in ''.join(w.get('label') or '' for w in ctl('ui').get('widgets', [])):
        click_label('Modo:', 1.0)
    close()
    zs = [c[2] for c in cells]
    stand = (ox, y, max(zs) + 3)
    front = (ox, y, max(zs))
    open_at(*front, stand)
    put('ntm:battery_creative', 1, 300)
    raw = hot('ntm:bedrock_ore_base')
    for s in raw:
        ctl('click-slot', s)
        time.sleep(0.3)
        ctl('click-slot', 302)
        time.sleep(0.4)
    t0 = time.time()
    while time.time() - t0 < 180:
        time.sleep(8)
        open_at(*front, stand)
        if not any(s[0] == 302 for s in machine_slots()):
            break
    out = machine_slots()
    ctl('screenshot', game / 'bedrock-slopper.png')
    for s in out:
        if 303 <= s[0] <= 308:
            ctl('click-slot', s[0], 'shift')
            time.sleep(0.4)
    close()
    evidence['slop'] = {'nucleo': [ox, oy, oz], 'saidas': out, 'segundos': round(time.time() - t0, 1),
                        'inventario': ctl('inventory').get('counts', {})}
    print('slop', evidence['slop'])


def centrifuge_run(cx, y, cz, item):
    """`item` (all of it) through the centrifuge; what comes out goes to the
    inventory."""
    stand = (cx, y, cz + 3)
    n = ctl('inventory').get('counts', {}).get(item, 0)
    open_at(cx, y, cz, stand)
    ctl('click-slot', hot(item)[0])
    time.sleep(0.3)
    ctl('click-slot', 300)
    time.sleep(0.5)
    t0 = time.time()
    while time.time() - t0 < 60 + 12 * n:
        time.sleep(5)
        open_at(cx, y, cz, stand)
        if not any(s[0] == 300 for s in machine_slots()):
            break
    time.sleep(11)
    open_at(cx, y, cz, stand)
    for s in machine_slots():
        if s[0] >= 301:
            ctl('click-slot', s[0], 'shift')
            time.sleep(0.4)
    close()


def centrifugar():
    y = floor_y()
    cx, cz = X + 4, Z + 10
    battery(cx + 1, y, cz, (cx + 1, y, cz + 3))
    place(cx, y, cz, 'ntm:machine_centrifuge', (cx, y, cz + 3))
    centrifuge_run(cx, y, cz, 'ntm:bedrock_ore__base_actinide')
    centrifuge_run(cx, y, cz, 'ntm:bedrock_ore__primary_actinide')
    counts = ctl('inventory').get('counts', {})
    evidence['centrifugar'] = {k: v for k, v in counts.items() if 'bedrock' in k or k in ('minecraft:gravel',)}
    print('centrifugar', evidence['centrifugar'])


def craftar():
    y = floor_y()
    # Room in the hotbar: everything but the uranium fragments out.
    for item in ctl('inventory').get('counts', {}):
        if item != 'ntm:bedrock_ore_fragment__uranium':
            ctl('command', f'clear @s {item}', check=False)
    tx, tz = X + 4, Z + 6
    place(tx, y, tz, 'minecraft:crafting_table', (tx, y, tz + 3))
    open_at(tx, y, tz, (tx, y, tz + 3))
    src = hot('ntm:bedrock_ore_fragment__uranium')
    ctl('click-slot', src[0])
    time.sleep(0.3)
    # One fragment a cell of the 3×3 grid (100..108), the rest back.
    for i in range(9):
        ctl('click-slot', 100 + i, 'right')
        time.sleep(0.2)
    ctl('click-slot', src[0])
    time.sleep(0.3)
    ctl('click-slot', 200, 'shift')
    time.sleep(1)
    close()
    evidence['craftar'] = {'po_de_uranio': ctl('inventory').get('counts', {}).get('ntm:powder_uranium', 0)}
    print('craftar', evidence['craftar'])


# --------------------------------------------------------- etapa 2

X3, Z3 = 2920, -1845  # the click: the core lands 3 cells north, at −1848


def count(item):
    return ctl('inventory').get('counts', {}).get(item, 0)


def fluid_barrel(bx, by, bz, barrel, stand):
    """A steel barrel at (bx, by, bz) with `barrel` poured in, in output
    mode."""
    # A full hotbar puts the next item in the main inventory, out of reach.
    ctl('command', 'clear @s', check=False)
    if not ctl('block', bx, by, bz).get('state', '').startswith('ntm:barrel_steel'):
        place(bx, by, bz, 'ntm:barrel_steel', stand)
    give(barrel, 1)
    ctl('select', barrel)
    close()
    tp(stand[0] + 0.5, stand[1], stand[2] + 0.5)
    ctl('look-at-block', bx, by, bz)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(2.5)
    click_label('Despejar')
    while 'Saída' not in ''.join(w.get('label') or '' for w in ctl('ui').get('widgets', [])):
        click_label('Modo:', 1.0)
    close()


def escavar3():
    y = floor_y()
    if not (game / 'bedrock-chao2').exists():
        ctl('fill', X3 - 6, y, Z3 - 12, X3 + 16, y + 9, Z3 + 6, 'minecraft:air')
        ctl('fill', X3 - 6, y - 1, Z3 - 12, X3 + 16, y - 1, Z3 + 6, 'minecraft:stone')
        (game / 'bedrock-chao2').write_text('1')
    ctl('command', 'clear @s', check=False)
    if not any('excavator' in b[3] for b in ctl('blocks', X3 - 4, y, Z3 - 7, X3 + 4, y + 6, Z3).get('blocks', [])):
        place(X3, y, Z3, 'ntm:machine_excavator', (X3, y, Z3 + 3))
    (cx, cy, cz) = find('ntm:machine_excavator', X3 - 4, y, Z3 - 7, X3 + 4, y + 6, Z3)
    ctl('fill', cx - 1, -63, cz - 1, cx + 1, cy - 9, cz + 1, 'minecraft:air')
    ctl('fill', cx, -64, cz, cx, -64, cz, 'ntm:ore_bedrock')
    # Sulfuric acid: a barrel on a pillar against the port layer (above the
    # core), the player on a second pillar.
    ctl('fill', cx + 4, y, cz, cx + 4, cy, cz, 'minecraft:stone')
    ctl('fill', cx + 6, y, cz, cx + 6, cy, cz, 'minecraft:stone')
    fluid_barrel(cx + 4, cy + 1, cz, 'ntm:fluid_barrel_full__sulfuric_acid', (cx + 6, cy + 1, cz))
    stand = (cx, y, cz + 6)
    open_at(cx, cy, cz + 3, stand)
    put('ntm:battery_creative', 1, 300)
    put('ntm:drillbit__desh', 1, 304)
    ctl('click-button', 'Broca')
    time.sleep(2)
    t0, raw = time.time(), []
    while time.time() - t0 < 900:
        time.sleep(15)
        open_at(cx, cy, cz + 3, stand)
        raw = [s for s in machine_slots() if s[1] == 'ntm:bedrock_ore_base']
        if sum(s[2] for s in raw) >= 2:
            break
    data = ctl('command', f'data get block {cx} {cy} {cz} Items').get('chat', [''])[0]
    ctl('click-button', 'Broca')
    close()
    evidence['escavar3'] = {'nucleo': [cx, cy, cz], 'brutos': sum(s[2] for s in raw), 'segundos': round(time.time() - t0, 1),
                            'custom_data_no_bloco': 'ntm_ore' in data and 'actinide: 2.2d' in data}
    print('escavar3', evidence['escavar3'])


def pilha():
    """The raw ores out by shift-click: the stack keeps its data."""
    y = floor_y()
    (cx, cy, cz) = find('ntm:machine_excavator', X3 - 4, y, Z3 - 7, X3 + 4, y + 6, Z3)
    ctl('command', 'clear @s', check=False)
    open_at(cx, cy, cz + 3, (cx, y, cz + 6))
    for s in machine_slots():
        if s[1] == 'ntm:bedrock_ore_base':
            ctl('click-slot', s[0], 'shift')
            time.sleep(0.5)
    close()
    inv = ctl('command', 'data get entity @s Inventory').get('chat', [''])[0]
    stacks = [(m.group(1) is not None, int(m.group(2))) for m in __import__('re').finditer(
        r'\{(components: \{[^}]*\{[^}]*\}\}\}, )?count: (\d+), Slot: \d+b, id: "ntm:bedrock_ore_base"\}', inv)]
    evidence['pilha'] = {'pilhas': stacks, 'ok': bool(stacks) and all(c for c, _ in stacks)}
    print('pilha', evidence['pilha'])


def crystallizer(px, pz, barrel):
    """A crystallizer clicked at (px, pz), fed by a barrel of `barrel` against
    its bottom ring; its core and the cell to open it through."""
    y = floor_y()
    if not any('crystallizer' in b[3] for b in ctl('blocks', px - 3, y, pz - 3, px + 3, y, pz + 3).get('blocks', [])):
        place(px, y, pz, 'ntm:machine_crystallizer', (px, y, pz + 4))
    (kx, ky, kz) = find('ntm:machine_crystallizer', px - 3, y, pz - 3, px + 3, y, pz + 3)
    fluid_barrel(kx + 2, y, kz, barrel, (kx + 4, y, kz))
    return (kx, ky, kz)


def crystallize(core, item, n, out_item):
    """`n` of `item` through the crystallizer at `core`; what comes out to the
    inventory."""
    (kx, y, kz) = core
    stand, cell = (kx, y, kz + 3), (kx, y, kz + 1)
    open_at(*cell, stand)
    put('ntm:battery_creative', 1, 301) if not any(s[0] == 301 for s in machine_slots()) else None
    ctl('click-slot', hot(item)[0])
    time.sleep(0.3)
    ctl('click-slot', 300)
    time.sleep(0.5)
    t0 = time.time()
    while time.time() - t0 < 30 + 15 * n:
        time.sleep(4)
        open_at(*cell, stand)
        if sum(s[2] for s in machine_slots() if s[1] == out_item) >= n // max(1, 1):
            break
    for s in machine_slots():
        if s[0] == 302:
            ctl('click-slot', 302, 'shift')
            time.sleep(0.4)
    close()


def lavar():
    y = floor_y()
    ctl('command', 'clear @s', check=False)
    # 2 BASE ores of actinides washed, 2 not.
    water = crystallizer(X3 + 10, Z3 - 8, 'ntm:fluid_barrel_full__water')
    give('ntm:bedrock_ore__base_actinide', 2)
    crystallize(water, 'ntm:bedrock_ore__base_actinide', 2, 'ntm:bedrock_ore__base_washed_actinide')
    washed = count('ntm:bedrock_ore__base_washed_actinide')
    give('ntm:bedrock_ore__base_actinide', 2)
    cx, cz = X + 4, Z + 10
    centrifuge_run(cx, y, cz, 'ntm:bedrock_ore__base_washed_actinide')
    from_washed = count('ntm:bedrock_ore__primary_actinide')
    centrifuge_run(cx, y, cz, 'ntm:bedrock_ore__base_actinide')
    from_base = count('ntm:bedrock_ore__primary_actinide') - from_washed
    evidence['lavar'] = {'lavados': washed, 'primary_de_2_lavados': from_washed, 'primary_de_2_base': from_base}
    print('lavar', evidence['lavar'])


def acido():
    y = floor_y()
    acid = crystallizer(X3 + 10, Z3, 'ntm:fluid_barrel_full__sulfuric_acid')
    n = count('ntm:bedrock_ore__primary_actinide')
    crystallize(acid, 'ntm:bedrock_ore__primary_actinide', n, 'ntm:bedrock_ore__primary_sulfuric_actinide')
    cx, cz = X + 4, Z + 10
    centrifuge_run(cx, y, cz, 'ntm:bedrock_ore__primary_sulfuric_actinide')
    centrifuge_run(cx, y, cz, 'ntm:bedrock_ore__primary_nosulfuric_actinide')
    water = find('ntm:machine_crystallizer', X3 + 7, y, Z3 - 11, X3 + 13, y, Z3 - 5)
    crystallize(water, 'ntm:bedrock_ore__sulfuric_byproduct_actinide', count('ntm:bedrock_ore__sulfuric_byproduct_actinide'),
                'ntm:bedrock_ore__sulfuric_washed_actinide')
    centrifuge_run(cx, y, cz, 'ntm:bedrock_ore__sulfuric_washed_actinide')
    evidence['acido'] = {k: v for k, v in ctl('inventory').get('counts', {}).items() if 'bedrock' in k}
    print('acido', evidence['acido'])


def recharge(bx, by, bz):
    """The machine battery of the centrifuge back to full (+10% ×12)."""
    close()
    tp(bx + 0.5, by, bz + 3.5)
    ctl('look-at-block', bx, by, bz)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(2.5)
    for _ in range(12):
        click_label('Teste: +10%', 1.0)
    close()


def fim():
    """What the acid line left: the no-sulfuric ores (in the centrifuge, its
    battery recharged) and the washed byproducts, to fragments; the dust."""
    y = floor_y()
    cx, cz = X + 4, Z + 10
    recharge(cx + 1, y, cz)
    # The no-sulfuric ores still in the centrifuge: let it run, take all.
    stand = (cx, y, cz + 3)
    t0 = time.time()
    while time.time() - t0 < 600:
        time.sleep(10)
        open_at(cx, y, cz, stand)
        left = [s for s in machine_slots() if s[0] == 300]
        for s in machine_slots():
            if s[0] >= 301:
                ctl('click-slot', s[0], 'shift')
                time.sleep(0.3)
        close()
        if not left:
            break
    water = find('ntm:machine_crystallizer', X3 + 7, y, Z3 - 11, X3 + 13, y, Z3 - 5)
    (kx, _, kz) = water
    open_at(kx, y, kz + 1, (kx, y, kz + 3))
    ctl('click-slot', 302, 'shift')
    time.sleep(0.5)
    close()
    recharge(cx + 1, y, cz)
    centrifuge_run(cx, y, cz, 'ntm:bedrock_ore__sulfuric_washed_actinide')
    evidence['fim'] = {k: v for k, v in ctl('inventory').get('counts', {}).items() if 'bedrock' in k}
    print('fim', evidence['fim'])


# --------------------------------------------------------- etapa 3

X4, Z4 = 2920, -1822  # a fresh floor north of stage 2


def footprint(block, x0, y0, z0, r):
    """The core of `block` near (x0, z0) and its bottom layer's box."""
    (cx, cy, cz) = find(block, x0 - r, y0, z0 - r, x0 + r, y0 + 3, z0 + r)
    cells = [b for b in ctl('blocks', cx - 7, y0, cz - 7, cx + 7, y0, cz + 7).get('blocks', []) if 'dummy' in b[3] or block in b[3]]
    xs, zs = [c[0] for c in cells], [c[2] for c in cells]
    return (cx, cy, cz), (min(xs), max(xs), min(zs), max(zs))


def barrel_state(bx, by, bz):
    """(fluid, mB) of the barrel at (bx, by, bz), from the mod's save."""
    ctl('save', check=False)
    time.sleep(12)
    # run-visual.sh: each game has its own copy of the mods and their data.
    d = game / 'dist/.horizon-mod-data/ntmlogic'
    f = max(d.glob('w.*'), key=lambda p: p.stat().st_mtime)
    t = f.read_text()
    w = json.loads(t[t.index('{'):])
    for m in w['machines']:
        if m['p'] == [bx, by, bz]:
            tank = m['m']['data'].get('tank', {})
            return tank.get('fluid', ''), tank.get('mb', 0)
    return '', 0


def old_screen(x, y, z, stand):
    close()
    tp(stand[0] + 0.5, stand[1], stand[2] + 0.5)
    ctl('look-at-block', x, y, z)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(2.5)


def carry(src, dst, n, mode_out, src_stand=None, dst_stand=None):
    """`n` × 1 000 mB from the barrel `src` to the barrel `dst` in fluid
    tanks (fill on one, pour on the other); `dst` then gives (output)."""
    for _ in range(n):
        give('ntm:fluid_tank_empty', 1)
        ctl('select', 'ntm:fluid_tank_empty')
        old_screen(*src, src_stand or (src[0], src[1], src[2] + 2))
        click_label('Encher')
        close()
        full = [k for k in ctl('inventory').get('counts', {}) if k.startswith('ntm:fluid_tank_full__')]
        if not full:
            raise SystemExit(f'o barril {src} não encheu o tanque')
        ctl('select', full[0])
        old_screen(*dst, (dst[0], dst[1], dst[2] + 2))
        click_label('Despejar')
        close()
    if mode_out:
        old_screen(*dst, (dst[0], dst[1], dst[2] + 2))
        while 'Saída' not in ''.join(w.get('label') or '' for w in ctl('ui').get('widgets', [])):
            click_label('Modo:', 1.0)
        close()


def torrar():
    y = floor_y()
    # Cleared once: a second fill would wipe the machines and barrels.
    if not (game / 'bedrock-chao3').exists():
        ctl('fill', X4 - 14, y, Z4 - 14, X4 + 14, y + 6, Z4 + 14, 'minecraft:air')
        ctl('fill', X4 - 14, y - 1, Z4 - 14, X4 + 14, y - 1, Z4 + 14, 'minecraft:stone')
        (game / 'bedrock-chao3').write_text('1')
    ctl('command', 'clear @s', check=False)
    ox, oz = X4 - 6, Z4
    if not any('pyrooven' in b[3] for b in ctl('blocks', ox - 4, y, oz - 4, ox + 4, y + 3, oz + 4).get('blocks', [])):
        place(ox, y, oz, 'ntm:machine_pyrooven', (ox, y, oz + 4))
    (core, (x0, x1, z0, z1)) = footprint('ntm:machine_pyrooven', ox, y, oz, 5)
    # Barrels against both long sides, at the core's row: the vitriol goes to
    # whichever touches a port.
    for bx in (x0 - 1, x1 + 1):
        if not ctl('block', bx, y, core[2]).get('state', '').startswith('ntm:barrel_steel'):
            place(bx, y, core[2], 'ntm:barrel_steel', (bx, y, core[2] + 2))
    stand, cell = (core[0], y, z1 + 3), (core[0], y, z1)
    open_at(*cell, stand)
    put('ntm:battery_creative', 1, 300)
    give('ntm:bedrock_ore__base_actinide', 40)
    ctl('click-slot', hot('ntm:bedrock_ore__base_actinide')[0])
    time.sleep(0.3)
    ctl('click-slot', 301)
    time.sleep(1)
    t0 = time.time()
    while time.time() - t0 < 120:
        time.sleep(5)
        open_at(*cell, stand)
        if not any(s[0] == 301 for s in machine_slots()):
            break
    time.sleep(3)
    roasted = sum(s[2] for s in machine_slots() if s[1] == 'ntm:bedrock_ore__base_roasted_actinide')
    ctl('click-slot', 302, 'shift')
    time.sleep(0.5)
    ctl('screenshot', game / 'bedrock-forno.png')
    close()
    barrels = {bx: barrel_state(bx, y, core[2]) for bx in (x0 - 1, x1 + 1)}
    # The roasted ores in the centrifuge: PRIMARY, as from BASE (once).
    if 'torrar' not in evidence:
        centrifuge_run(X + 4, y, Z + 10, 'ntm:bedrock_ore__base_roasted_actinide')
    first = evidence.get('torrar', {})
    evidence['torrar'] = {**first, 'nucleo': core, 'torrados': roasted, 'barris': barrels,
                          'primary_dos_torrados': first.get('primary_dos_torrados', count('ntm:bedrock_ore__primary_actinide'))}
    print('torrar', evidence['torrar'])


def eletrolisar():
    y = floor_y()
    ex, ez = X4 + 7, Z4
    if not any('electrolyser' in b[3] for b in ctl('blocks', ex - 6, y, ez - 6, ex + 6, y + 3, ez + 6).get('blocks', [])):
        place(ex, y, ez, 'ntm:machine_electrolyser', (ex, y, ez + 4))
    (core, (x0, x1, z0, z1)) = footprint('ntm:machine_electrolyser', ex, y, ez, 7)
    stand, cell = (core[0], y, z1 + 3), (core[0], y, z1)
    open_at(*cell, stand)
    put('ntm:battery_creative', 1, 300)
    close()
    # The oven's vitriol (whichever barrel got it) into a barrel at the
    # electrolyser's port; the output barrel only after it went in (an empty
    # barrel takes any fluid, the vitriol too).
    oven = evidence['torrar']['barris']
    src_x = max(oven, key=lambda k: oven[k][1])
    (_, _, ocz) = evidence['torrar']['nucleo']
    src = (int(src_x), y, ocz)
    feed = (x0 - 1, y, core[2])
    if not ctl('block', *feed).get('state', '').startswith('ntm:barrel_steel'):
        place(*feed, 'ntm:barrel_steel', (feed[0], y, feed[2] + 2))
    carry(src, feed, 2, True)
    time.sleep(10)
    # The acid's barrel, typed by an acid tank in the hand (chlorine comes
    # first in the networks' order and would take an untyped one).
    out = (x1 + 1, y, core[2])
    if not ctl('block', *out).get('state', '').startswith('ntm:barrel_steel'):
        place(*out, 'ntm:barrel_steel', (out[0], y, out[2] + 2))
    give('ntm:fluid_tank_full__sulfuric_acid', 1)
    ctl('select', 'ntm:fluid_tank_full__sulfuric_acid')
    old_screen(*out, (out[0], y, out[2] + 2))
    click_label('Tipo pelo recipiente')
    close()
    ctl('command', 'clear @s ntm:fluid_tank_full__sulfuric_acid', check=False)
    time.sleep(10)
    evidence['eletrolisar'] = {'nucleo': core, 'entrada': barrel_state(*feed), 'acido': barrel_state(*out), 'barris_saida': [out]}
    open_at(*cell, stand)
    evidence['eletrolisar']['subprodutos'] = [s for s in machine_slots() if s[0] >= 311]
    ctl('screenshot', game / 'bedrock-eletrolisador.png')
    close()
    print('eletrolisar', evidence['eletrolisar'])


def ciclo():
    y = floor_y()
    acid = tuple(evidence['eletrolisar']['barris_saida'][0])
    states = {acid: barrel_state(*acid)}
    # A new crystallizer whose only fluid is the electrolyser's acid.
    px, pz = X4 + 2, Z4 + 9
    if not any('crystallizer' in b[3] for b in ctl('blocks', px - 3, y, pz - 3, px + 3, y, pz + 3).get('blocks', [])):
        place(px, y, pz, 'ntm:machine_crystallizer', (px, y, pz + 4))
    (kx, ky, kz) = find('ntm:machine_crystallizer', px - 3, y, pz - 3, px + 3, y, pz + 3)
    dst = (kx + 2, y, kz)
    if not ctl('block', *dst).get('state', '').startswith('ntm:barrel_steel'):
        place(*dst, 'ntm:barrel_steel', (dst[0] + 2, y, kz))
    carry(acid, dst, 1, True)
    give('ntm:bedrock_ore__primary_actinide', 2)
    crystallize((kx, y, kz), 'ntm:bedrock_ore__primary_actinide', 2, 'ntm:bedrock_ore__primary_sulfuric_actinide')
    evidence['ciclo'] = {'acido_do_eletrolisador': states[acid], 'sulfuricos': count('ntm:bedrock_ore__primary_sulfuric_actinide')}
    print('ciclo', evidence['ciclo'])


# --------------------------------------------------------- etapa 4

DIRS = {'north': (0, -1), 'south': (0, 1), 'east': (1, 0), 'west': (-1, 0)}


def facing(x, y, z):
    st = ctl('block', x, y, z).get('state', '')
    return DIRS[st.split('facing=')[1].split(',')[0].split(']')[0]]


def mold_at(x, y, z, stand):
    """An ingot mold on (x, y, z), its mold in by shift-click."""
    if not ctl('block', x, y, z).get('state', '').startswith('ntm:foundry_mold'):
        place(x, y, z, 'ntm:foundry_mold', stand)
    open_at(x, y, z, stand)
    if not any(s[0] == 300 for s in machine_slots()):
        give('ntm:mold__ingot', 1)
        open_at(x, y, z, stand)
        ctl('click-slot', hot('ntm:mold__ingot')[0], 'shift')
        time.sleep(0.7)
    got = machine_slots()
    close()
    return got


def casts(x, y, z, stand):
    """What the mold's output (machine slot 1) holds, taken out."""
    open_at(x, y, z, stand)
    out = [s for s in machine_slots() if s[0] == 301]
    for s in out:
        ctl('click-slot', 301, 'shift')
        time.sleep(0.5)
    close()
    return [(s[1], s[2]) for s in out]


def hidrogenar():
    ctl('command', 'clear @s', check=False)
    h2 = crystallizer(X4 - 12, Z4 - 10, 'ntm:fluid_barrel_full__hydrogen')
    # PRIMARY ores as the centrifuge gave them in stages 1 and 2.
    give('ntm:bedrock_ore__primary_actinide', 3)
    crystallize(h2, 'ntm:bedrock_ore__primary_actinide', 3, 'ntm:bedrock_ore__primary_first_actinide')
    evidence['hidrogenar'] = {'nucleo': h2, 'primary_first': count('ntm:bedrock_ore__primary_first_actinide')}
    print('hidrogenar', evidence['hidrogenar'])


def metal():
    y = floor_y()
    core = tuple(evidence['eletrolisar']['nucleo'])
    (dx, dz) = facing(*core)
    # Left 6 behind the core, right 6 ahead (TileEntityElectrolyser:201-245).
    molds = {side: (core[0] + dx * k, y, core[2] + dz * k) for side, k in (('esquerda', -6), ('direita', 6))}
    # Where to stand by a mold: beside it, off the machine's line.
    beside = {side: (m[0] + 2, y, m[2]) if dx == 0 else (m[0], y, m[2] + 2) for side, m in molds.items()}
    for side, m in molds.items():
        mold_at(*m, beside[side])
    # Nitric acid into the stage's (empty) feed barrel, typed by the barrel
    # in the hand; the electrolyser takes it into its acid tank.
    feed = (core[0] - 2, y, core[2]) if dx == 0 else (core[0], y, core[2] - 2)
    give('ntm:fluid_barrel_full__nitric_acid', 1)
    ctl('select', 'ntm:fluid_barrel_full__nitric_acid')
    old_screen(*feed, (feed[0], y, feed[2] + 2) if dx == 0 else (feed[0] + 2, y, feed[2]))
    click_label('Tipo pelo recipiente')
    click_label('Despejar')
    while 'Saída' not in ''.join(w.get('label') or '' for w in ctl('ui').get('widgets', [])):
        click_label('Modo:', 1.0)
    close()
    # Its side, off the molds' line.
    cell, stand = ((core[0] - 1, y, core[2] + 2), (core[0] - 4, y, core[2] + 2)) if dx == 0 else ((core[0] + 2, y, core[2] - 1), (core[0] + 2, y, core[2] - 4))
    open_at(*cell, stand)
    if not any(s[0] == 300 for s in machine_slots()):
        put('ntm:battery_creative', 1, 300)
    # One of the crystallizer's PRIMARY FIRST (the rest for the arc).
    ore = 'ntm:bedrock_ore__primary_first_actinide'
    src = hot(ore)[0]
    ctl('click-slot', src)
    time.sleep(0.3)
    ctl('click-slot', 314, 'right')
    time.sleep(0.5)
    ctl('click-slot', src)
    time.sleep(0.3)
    put_in = [s for s in machine_slots() if s[0] == 314]
    close()
    t0, got = time.time(), {}
    while time.time() - t0 < 120:
        time.sleep(6)
        for side, m in molds.items():
            for item, n in casts(*m, beside[side]):
                got.setdefault(side, {}).setdefault(item, 0)
                got[side][item] += n
        if sum(got.get('esquerda', {}).values()) >= 3 and got.get('direita'):
            break
    open_at(*cell, stand)
    left = machine_slots()
    ctl('screenshot', game / 'bedrock-eletrolisador-metal.png')
    close()
    evidence['metal'] = {'nucleo': core, 'moldes': molds, 'posto': put_in, 'lingotes': got, 'slots': left, 'barril_nitrico': barrel_state(*feed)}
    print('metal', evidence['metal'])


def arco():
    y = floor_y()
    ax, az = X4 - 8, Z4 + 9
    if not any('arc_furnace' in b[3] for b in ctl('blocks', ax - 4, y, az - 4, ax + 4, y + 4, az + 4).get('blocks', [])):
        place(ax, y, az, 'ntm:machine_arc_furnace_large', (ax, y, az + 4))
    core = find('ntm:machine_arc_furnace_large', ax - 4, y, az - 4, ax + 4, y + 4, az + 4)
    (dx, dz) = facing(*core)
    # The spout 3 ahead of the core, a block up (…ArcFurnaceLarge:166-171).
    m = (core[0] + 3 * dx, y, core[2] + 3 * dz)
    m_stand = (m[0] + 2, y, m[2]) if dx == 0 else (m[0], y, m[2] + 2)
    mold_at(*m, m_stand)
    # Its back edge (the core is inside), from behind.
    cell, stand = (core[0] - 2 * dx, y, core[2] - 2 * dz), (core[0] - 4 * dx, y, core[2] - 4 * dz)
    open_at(*cell, stand)
    if not any(s[0] == 303 for s in machine_slots()):
        put('ntm:battery_creative', 1, 303)
    for e in (300, 301, 302):
        if not any(s[0] == e for s in machine_slots()):
            put('ntm:arc_electrode__graphite', 1, e)
    labels = [w.get('label') for w in ctl('ui').get('widgets', [])]
    # A new furnace is in solid mode: its button once.
    if not (game / 'bedrock-arco-modo').exists():
        click_label('Modo', 1.0)
        (game / 'bedrock-arco-modo').write_text('1')
    ctl('click-slot', hot('ntm:bedrock_ore__primary_first_actinide')[0])
    time.sleep(0.3)
    ctl('click-slot', 325)
    time.sleep(0.5)
    before = machine_slots()
    ctl('screenshot', game / 'bedrock-arco-carregado.png')
    close()
    t0, got = time.time(), {}
    while time.time() - t0 < 150:
        time.sleep(8)
        for item, n in casts(*m, m_stand):
            got[item] = got.get(item, 0) + n
        if got.get('ntm:ingot_uranium', 0) >= 4:
            break
    open_at(*cell, stand)
    after = machine_slots()
    ctl('screenshot', game / 'bedrock-arco.png')
    close()
    evidence['arco'] = {'nucleo': core, 'molde': m, 'botoes': labels, 'antes': before, 'lingotes': got, 'depois': after}
    print('arco', evidence['arco'])


# --------------------------------------------- escavadora: anéis e chaves

X5, Z5 = 2960, -1820


def stone_floor():
    """Stone down to 14 below the floor round X5 (once)."""
    y = floor_y()
    if not (game / 'bedrock-chao5').exists():
        tp(X5 + 0.5, y + 2, Z5 + 0.5)
        time.sleep(3)
        ctl('fill', X5 - 10, y, Z5 - 10, X5 + 34, y + 8, Z5 + 10, 'minecraft:air')
        for top in (y - 1, y - 8):
            ctl('fill', X5 - 10, top - 6, Z5 - 10, X5 + 34, top, Z5 + 10, 'minecraft:stone')
        (game / 'bedrock-chao5').write_text('1')
    return y


def excavator_at(x, z, drill, extra, buttons):
    """An excavator clicked at (x, z): its core, the cell and stand to open
    it, with the battery, `drill`, `extra` (slot, item) and `buttons`."""
    y = floor_y()
    if not any('excavator' in b[3] for b in ctl('blocks', x - 4, y, z - 6, x + 4, y + 6, z + 1).get('blocks', [])):
        place(x, y, z, 'ntm:machine_excavator', (x, y, z + 3))
    core = find('ntm:machine_excavator', x - 4, y, z - 4, x + 4, y + 6, z + 4)
    cell, stand = (core[0], core[1], core[2] + 3), (core[0], y, core[2] + 6)
    open_at(*cell, stand)
    have = {sl[0] for sl in machine_slots()}
    for slot, item in [(300, 'ntm:battery_creative'), (304, drill)] + extra:
        if slot not in have:
            put(item, 1, slot)
    for b in buttons:
        ctl('click-button', b)
        time.sleep(0.8)
    close()
    return core, cell, stand


def dig(core, cell, stand, seconds):
    """Lets it dig, then turns it off: its buffer."""
    time.sleep(seconds)
    open_at(*cell, stand)
    buf = machine_slots()
    ctl('screenshot', game / 'bedrock-escavadora-chaves.png')
    ctl('click-button', 'Broca')
    time.sleep(1)
    close()
    return buf


def aneis():
    y = stone_floor()
    ctl('command', 'clear @s', check=False)
    # The vein, 4–6 east of the axis on the 3rd layer (radius 3: the drill
    # mines the ore just past the ring; the vein miner the rest).
    x, z = X5, Z5
    core, cell, stand = excavator_at(x, z, 'ntm:drillbit__ferro', [(302, 'ntm:upgrade_effect_1')], [])
    (cx, cy, cz) = core
    vy = cy - 6
    for dx in (4, 5, 6):
        ctl('fill', cx + dx, vy, cz, cx + dx, vy, cz, 'minecraft:iron_ore')
    open_at(*cell, stand)
    for b in ('Britador', 'Veio', 'Broca'):
        ctl('click-button', b)
        time.sleep(0.8)
    close()
    buf = dig(core, cell, stand, 25)
    blocks = {k: ctl('block', *p).get('state', '') for k, p in {
        'canto_7x7': (cx + 3, vy, cz + 3), 'fora_7x7': (cx + 4, vy, cz + 1),
        'primeira_camada_fora_3x3': (cx + 2, cy - 4, cz), 'veia': (cx + 6, vy, cz)}.items()}
    evidence['aneis'] = {'nucleo': core, 'blocos': blocks, 'buffer': buf}
    print('aneis', evidence['aneis'])


def seda():
    y = stone_floor()
    ctl('command', 'clear @s', check=False)
    core, cell, stand = excavator_at(X5 + 22, Z5, 'ntm:drillbit__steel_diamond', [], ['Toque de seda', 'Broca'])
    buf = dig(core, cell, stand, 20)
    evidence['seda'] = {'nucleo': core, 'buffer': buf}
    print('seda', evidence['seda'])


def paredes():
    y = stone_floor()
    ctl('command', 'clear @s', check=False)
    x, z = X5 + 11, Z5 - 1
    core, cell, stand = excavator_at(x, z, 'ntm:drillbit__ferro', [], [])
    (cx, cy, cz) = core
    vy = cy - 6
    ctl('fill', cx + 2, vy, cz, cx + 2, vy, cz, 'minecraft:water')
    ctl('fill', cx - 2, vy, cz + 1, cx - 2, vy, cz + 1, 'minecraft:air')
    open_at(*cell, stand)
    for b in ('Paredes', 'Broca'):
        ctl('click-button', b)
        time.sleep(0.8)
    close()
    buf = dig(core, cell, stand, 15)
    blocks = {k: ctl('block', *p).get('state', '') for k, p in {
        'agua_na_borda': (cx + 2, vy, cz), 'ar_na_borda': (cx - 2, vy, cz + 1),
        'borda_de_pedra': (cx + 2, vy, cz + 2), 'dentro': (cx + 1, vy, cz)}.items()}
    evidence['paredes'] = {'nucleo': core, 'blocos': blocks, 'buffer': buf}
    print('paredes', evidence['paredes'])


def machine_state(x, y, z):
    """The mod's save of the machine at (x, y, z)."""
    ctl('save', check=False)
    time.sleep(12)
    d = game / 'dist/.horizon-mod-data/ntmlogic'
    f = max(d.glob('w.*'), key=lambda p: p.stat().st_mtime)
    t = f.read_text()
    w = json.loads(t[t.index('{'):])
    for m in w['machines']:
        if m['p'] == [x, y, z]:
            return m['m']['data']
    return None


def petroleo_b():
    y = stone_floor()
    ctl('command', 'clear @s', check=False)
    # The refinery, a hot oil barrel against its north-west corner port.
    rx, rz = X5 + 15, Z5 + 6
    if not any('refinery' in b[3] for b in ctl('blocks', rx - 3, y, rz - 3, rx + 3, y + 2, rz + 3).get('blocks', [])):
        place(rx, y, rz, 'ntm:machine_refinery', (rx, y, rz + 3))
    ref = find('ntm:machine_refinery', rx - 3, y, rz - 3, rx + 3, y + 2, rz + 3)
    fluid_barrel(ref[0] - 2, y, ref[2] - 1, 'ntm:fluid_barrel_full__hotoil', (ref[0] - 4, y, ref[2] - 1))
    open_at(ref[0], y, ref[2] + 1, (ref[0], y, ref[2] + 3))
    if not any(sl[0] == 300 for sl in machine_slots()):
        put('ntm:battery_creative', 1, 300)
    close()
    # Two fraction towers, one on the other; heavy oil into the bottom one.
    tx, tz = X5 + 21, Z5 + 6
    if not any('fraction_tower' in b[3] for b in ctl('blocks', tx - 2, y, tz - 2, tx + 2, y + 2, tz + 2).get('blocks', [])):
        place(tx, y, tz, 'ntm:machine_fraction_tower', (tx, y, tz + 3))
    low = find('ntm:machine_fraction_tower', tx - 2, y, tz - 2, tx + 2, y + 2, tz + 2)
    if not ctl('block', low[0], y + 3, low[2]).get('state', '').startswith('ntm:machine_fraction_tower'):
        # On top of the lower one, from the ground: the core lands a block
        # off the click, as the lower one's did.
        # The lower one's top is seen from above: a pillar to stand on.
        pillar = (low[0], y, low[2] + 4, low[0], y + 2, low[2] + 4)
        ctl('fill', *pillar, 'minecraft:stone')
        place(low[0], y + 3, low[2] + 1, 'ntm:machine_fraction_tower', (low[0], y + 3, low[2] + 4))
        ctl('fill', *pillar, 'minecraft:air')
    if not ctl('block', low[0], y + 3, low[2]).get('state', '').startswith('ntm:machine_fraction_tower'):
        raise SystemExit(f'a torre de cima não ficou sobre a de baixo: {ctl("blocks", low[0] - 1, y + 3, low[2] - 2, low[0] + 1, y + 3, low[2] + 2)}')
    fluid_barrel(low[0] + 2, y, low[2], 'ntm:fluid_barrel_full__heavyoil', (low[0] + 4, y, low[2]))
    # The mixer: coolant picked from the list, water from a barrel, niter.
    mx, mz = X5 + 27, Z5 + 6
    if not ctl('block', mx, y, mz).get('state', '').startswith('ntm:machine_mixer'):
        place(mx, y, mz, 'ntm:machine_mixer', (mx, y, mz + 3))
    mix = find('ntm:machine_mixer', mx - 1, y, mz - 1, mx + 1, y, mz + 1)
    open_at(*mix, (mix[0], y, mix[2] + 3))
    if not any(sl[0] == 300 for sl in machine_slots()):
        put('ntm:battery_creative', 1, 300)
    ctl('click-button', 'Receita')
    time.sleep(2)
    # The list's rows aren't widgets: coolant is its first row (anvil.py).
    ctl('click-at', 150, 50)
    time.sleep(1)
    close()
    fluid_barrel(mix[0] + 1, y, mix[2], 'ntm:fluid_barrel_full__water', (mix[0] + 3, y, mix[2]))
    open_at(*mix, (mix[0], y, mix[2] + 3))
    put('ntm:niter', 4, 301)
    close()
    time.sleep(30)
    open_at(ref[0], y, ref[2] + 1, (ref[0], y, ref[2] + 3))
    solid = [sl for sl in machine_slots() if sl[0] == 311]
    ctl('screenshot', game / 'petroleo-refinaria.png')
    close()
    tank = lambda t: (t.get('fluid', ''), t.get('mb', 0))
    r = machine_state(*ref) or {}
    lo, hi = machine_state(*low) or {}, machine_state(low[0], y + 3, low[2]) or {}
    m = machine_state(*mix) or {}
    evidence['petroleo_b'] = {
        'refinaria': {'nucleo': ref, 'entrada': tank(r.get('input', {})), 'saidas': [tank(t) for t in r.get('outputs', [])], 'enxofre': solid},
        'torres': {'baixo': [tank(t) for t in lo.get('tanks', [])], 'cima': [tank(t) for t in hi.get('tanks', [])]},
        'misturador': {'receita': m.get('recipe'), 'entradas': [tank(t) for t in m.get('tin', [])], 'saida': [tank(t) for t in m.get('tout', [])]},
    }
    print('petroleo_b', evidence['petroleo_b'])


X7, Z7 = 3010, -1800


def oil_floor_c():
    """A stone floor cleared 25 high at X7 (once)."""
    y = floor_y()
    if not (game / 'bedrock-chao7').exists():
        tp(X7 + 30.5, y + 2, Z7 + 0.5)
        time.sleep(4)
        for x0 in range(X7 - 6, X7 + 66, 10):
            ctl('fill', x0, y, Z7 - 12, x0 + 9, y + 24, Z7 + 12, 'minecraft:air')
            ctl('fill', x0, y - 3, Z7 - 12, x0 + 9, y - 1, Z7 + 12, 'minecraft:stone')
        (game / 'bedrock-chao7').write_text('1')
    return y


def on_firebox(x, z, block):
    """A firebox on the floor at (x, z) with coal, and `block` on it (placed
    from a pillar: the firebox's top is seen from above); both cores."""
    y = floor_y()
    if not any('heater_firebox' in b[3] for b in ctl('blocks', x - 2, y, z - 3, x + 2, y, z + 1).get('blocks', [])):
        place(x, y, z, 'ntm:heater_firebox', (x, y, z + 3))
    fire = find('ntm:heater_firebox', x - 2, y, z - 3, x + 2, y, z + 1)
    open_at(fire[0], y, fire[2] + 1, (fire[0], y, fire[2] + 3))
    if not any(sl[0] == 300 for sl in machine_slots()):
        put('minecraft:coal', 64, 300)
    close()
    if not ctl('block', fire[0], y + 1, fire[2]).get('state', '').startswith(block):
        # Two from the click (reach): 4 blocks away it was out of reach.
        pillar = (x, y, z + 2, x, y + 2, z + 2)
        ctl('fill', *pillar, 'minecraft:stone')
        close()
        tp(x + 0.5, y + 3, z + 2.5)
        give(block, 1)
        # Sneaking: a plain click on the firebox opens its screen.
        ctl('key', 'shift', 'down')
        ctl('place', x, y + 1, z, '--item', block, check=False)
        ctl('key', 'shift', 'up')
        time.sleep(2)
        close()
        ctl('fill', *pillar, 'minecraft:air')
    core = (fire[0], y + 1, fire[2])
    if not ctl('block', *core).get('state', '').startswith(block):
        raise SystemExit(f'{block} fora de cima do firebox {fire}')
    return fire, core


def barrel_up(bx, by, bz, barrel):
    """A barrel at (bx, by, bz), one up from the floor, on a stone; with
    `barrel` poured in (output), or empty (input) when None."""
    y = floor_y()
    if by > y:
        ctl('fill', bx, y, bz, bx, by - 1, bz, 'minecraft:stone')
    if barrel:
        fluid_barrel(bx, by, bz, barrel, (bx, y, bz + 2))
    elif not ctl('block', bx, by, bz).get('state', '').startswith('ntm:barrel_steel'):
        place(bx, by, bz, 'ntm:barrel_steel', (bx, y, bz + 2))


def petroleo_c():
    y = oil_floor_c()
    ctl('command', 'clear @s', check=False)
    tank = lambda t: (t.get('fluid', ''), t.get('mb', 0))
    ev = {}
    # The boiler on a firebox: water in from the west side, steam out east.
    fire, boil = on_firebox(X7, Z7, 'ntm:machine_boiler')
    barrel_up(boil[0] - 2, y + 1, boil[2], 'ntm:fluid_barrel_full__water')
    barrel_up(boil[0] + 2, y + 1, boil[2], None)
    # The coker on another: heavy oil from the west.
    fire2, cok = on_firebox(X7 + 10, Z7, 'ntm:machine_coker')
    barrel_up(cok[0] - 2, y + 1, cok[2], 'ntm:fluid_barrel_full__heavyoil')
    # The cracker: oil west, steam east (its bottom layer: x -2..3).
    cx = X7 + 22
    if not any('catalytic_cracker' in b[3] for b in ctl('blocks', cx - 4, y, Z7 - 6, cx + 4, y, Z7 + 2).get('blocks', [])):
        place(cx, y, Z7, 'ntm:machine_catalytic_cracker', (cx, y, Z7 + 3))
    crk = find('ntm:machine_catalytic_cracker', cx - 4, y, Z7 - 6, cx + 4, y, Z7 + 2)
    fluid_barrel(crk[0] - 3, y, crk[2], 'ntm:fluid_barrel_full__oil', (crk[0] - 5, y, crk[2]))
    fluid_barrel(crk[0] + 4, y, crk[2], 'ntm:fluid_barrel_full__steam', (crk[0] + 6, y, crk[2]))
    # The electric three: battery, catalyst, their fluids from barrels.
    elec = {}
    for name, block, x, west, east, cat, half in (
            ('reformador', 'ntm:machine_catalytic_reformer', X7 + 34, 'ntm:fluid_barrel_full__lightoil', None, True, 2),
            ('hidrotratador', 'ntm:machine_hydrotreater', X7 + 44, 'ntm:fluid_barrel_full__oil', 'ntm:fluid_barrel_full__hydrogen', True, 1),
            ('destilador', 'ntm:machine_vacuum_distill', X7 + 54, 'ntm:fluid_barrel_full__oil', None, False, 1)):
        if not any(block in b[3] for b in ctl('blocks', x - 3, y, Z7 - 3, x + 3, y, Z7 + 2).get('blocks', [])):
            place(x, y, Z7, block, (x, y, Z7 + 3))
        core = find(block, x - 3, y, Z7 - 3, x + 3, y, Z7 + 2)
        open_at(core[0], y, core[2] + 1, (core[0], y, core[2] + 3))
        have = {sl[0] for sl in machine_slots()}
        if 300 not in have:
            put('ntm:battery_creative', 1, 300)
        if cat and 310 not in have:
            put('ntm:catalytic_converter', 1, 310)
        close()
        fluid_barrel(core[0] - half - 1, y, core[2], west, (core[0] - half - 3, y, core[2]))
        if east:
            fluid_barrel(core[0] + half + 1, y, core[2], east, (core[0] + half + 3, y, core[2]))
        elec[name] = core
    time.sleep(40)
    b = machine_state(*boil) or {}
    ev['caldeira'] = {'nucleo': boil, 'calor': b.get('heat'), 'entrada': tank(b.get('input', {})), 'saida': tank(b.get('output', {})),
                      'barril_vapor': barrel_state(boil[0] + 2, y + 1, boil[2])}
    open_at(cok[0], y + 1, cok[2] + 1, (cok[0], y, cok[2] + 3))
    coke = [sl for sl in machine_slots() if sl[0] == 301]
    ctl('screenshot', game / 'petroleo-coqueificadora.png')
    close()
    c = machine_state(*cok) or {}
    ev['coqueificadora'] = {'nucleo': cok, 'calor': c.get('heat'), 'entrada': tank(c.get('input', {})), 'subproduto': tank(c.get('output', {})), 'coque': coke}
    k = machine_state(*crk) or {}
    ev['craqueador'] = {'nucleo': crk, 'entradas': [tank(t) for t in k.get('inputs', [])], 'saidas': [tank(t) for t in k.get('outputs', [])]}
    for name, core in elec.items():
        m = machine_state(*core) or {}
        ev[name] = {'nucleo': core, 'energia': m.get('power'), 'entradas': [tank(t) for t in m.get('inputs', [])], 'saidas': [tank(t) for t in m.get('outputs', [])]}
    evidence['petroleo_c'] = ev
    print('petroleo_c', json.dumps(ev, ensure_ascii=False))


def typed_barrel(pos, tank_item, stand=None):
    """An empty barrel at `pos` typed by a full tank in the hand (input)."""
    stand = stand or (pos[0], pos[1], pos[2] + 2)
    if not ctl('block', *pos).get('state', '').startswith('ntm:barrel_steel'):
        place(*pos, 'ntm:barrel_steel', stand)
    ctl('command', 'clear @s', check=False)
    give(tank_item, 1)
    ctl('select', tank_item)
    old_screen(*pos, stand)
    click_label('Tipo pelo recipiente')
    close()
    ctl('command', 'clear @s', check=False)


def mixer_pick(core, row):
    """The mixer's recipe: its list scrolled `row` rows, the top one."""
    open_at(*core, (core[0], core[1], core[2] + 3))
    if not any(sl[0] == 300 for sl in machine_slots()):
        put('ntm:battery_creative', 1, 300)
    ctl('click-button', 'Receita')
    time.sleep(2)
    ctl('scroll-at', 150, 80, -row)
    time.sleep(1)
    ctl('click-at', 150, 50)
    time.sleep(1)
    close()


def etapa5():
    y = oil_floor_c()
    ctl('command', 'clear @s', check=False)
    # Its chunks loaded (the step may start far away).
    tp(X7 + 34.5, y, Z7 + 4.5)
    time.sleep(4)
    tank = lambda t: (t.get('fluid', ''), t.get('mb', 0))
    # Aromatics and reformgas from the reformer (south of it), naphtha from
    # the refinery (its south-east corner port).
    ref = find('ntm:machine_catalytic_reformer', X7 + 31, y, Z7 - 4, X7 + 37, y, Z7 + 2)
    aro, rgas = (ref[0] - 1, y, ref[2] + 2), (ref[0] + 1, y, ref[2] + 2)
    typed_barrel(aro, 'ntm:fluid_tank_full__aromatics')
    typed_barrel(rgas, 'ntm:fluid_tank_full__reformgas')
    # Typed only once in place: what came first went when it was typed. The
    # reformer runs again on more light oil (its west barrel) to fill them.
    fluid_barrel(ref[0] - 3, y, ref[2], 'ntm:fluid_barrel_full__lightoil', (ref[0] - 5, y, ref[2]))
    time.sleep(12)
    rfy = find('ntm:machine_refinery', X5 + 12, y, Z5 + 3, X5 + 18, y, Z5 + 9)
    nap = (rfy[0] + 2, y, rfy[2] + 1)
    typed_barrel(nap, 'ntm:fluid_tank_full__naphtha')
    time.sleep(8)
    got = {'aromaticos': barrel_state(*aro), 'reformgas': barrel_state(*rgas), 'nafta': barrel_state(*nap)}
    # Two mixers: solvent (row 15) and radiosolvent (row 21, with chlorine).
    mixers = {}
    for name, x, row, west, east, out in (
            ('solvente', X7 + 30, 15, nap, aro, 'solvent'),
            ('radiossolvente', X7 + 36, 21, rgas, None, 'radiosolvent')):
        if not ctl('block', x, y, Z7 + 8).get('state', '').startswith('ntm:machine_mixer'):
            place(x, y, Z7 + 8, 'ntm:machine_mixer', (x, y, Z7 + 11))
        m = find('ntm:machine_mixer', x - 1, y, Z7 + 6, x + 1, y, Z7 + 9)
        mixer_pick(m, row)
        w, e, o = (m[0] - 1, y, m[2]), (m[0] + 1, y, m[2]), (m[0], y, m[2] - 1)
        typed_barrel(o, f'ntm:fluid_tank_full__{out}', (m[0] + 2, y, m[2] - 2))
        for pos in (w, e):
            if not ctl('block', *pos).get('state', '').startswith('ntm:barrel_steel'):
                place(*pos, 'ntm:barrel_steel', (pos[0], y, pos[2] + 2))
        carry(west, w, 1, True)
        if east:
            carry(east, e, 1, True)
        else:
            give('ntm:fluid_tank_full__chlorine', 1)
            ctl('select', 'ntm:fluid_tank_full__chlorine')
            old_screen(*e, (e[0], y, e[2] + 2))
            click_label('Despejar')
            while 'Saída' not in ''.join(w2.get('label') or '' for w2 in ctl('ui').get('widgets', [])):
                click_label('Modo:', 1.0)
            close()
        mixers[name] = (m, o)
    time.sleep(10)
    made = {n: barrel_state(*o) for n, (m, o) in mixers.items()}
    # Each solvent into a crystallizer of its own: PRIMARY → its PRIMARY.
    results = {}
    for (name, (m, o)), x, out_item in zip(mixers.items(), (X7 + 40, X7 + 46),
                                          ('ntm:bedrock_ore__primary_solvent_actinide', 'ntm:bedrock_ore__primary_rad_actinide')):
        if not any('crystallizer' in b[3] for b in ctl('blocks', x - 2, y, Z7 - 11, x + 2, y, Z7 - 7).get('blocks', [])):
            place(x, y, Z7 - 8, 'ntm:machine_crystallizer', (x, y, Z7 - 5))
        k = find('ntm:machine_crystallizer', x - 2, y, Z7 - 11, x + 2, y, Z7 - 7)
        kb = (k[0] + 2, y, k[2])
        if not ctl('block', *kb).get('state', '').startswith('ntm:barrel_steel'):
            place(*kb, 'ntm:barrel_steel', (kb[0] + 2, y, kb[2]))
        # The mixer stands south of its output barrel: open it from the east.
        carry(o, kb, 1, True, src_stand=(o[0] + 2, y, o[2]))
        # The stage 1 centrifuge: its battery full, its slots empty.
        cx, cz = X + 4, Z + 10
        recharge(cx + 1, y, cz)
        open_at(cx, y, cz, (cx, y, cz + 3))
        for sl in machine_slots():
            if sl[0] >= 300:
                ctl('click-slot', sl[0], 'shift')
                time.sleep(0.3)
        close()
        ctl('command', 'clear @s', check=False)
        give('ntm:bedrock_ore__primary_actinide', 2)
        crystallize(k, 'ntm:bedrock_ore__primary_actinide', 2, out_item)
        results[name] = count(out_item)
        # The centrifuge splits it (the stage 1 one).
        if results[name]:
            centrifuge_run(X + 4, y, Z + 10, out_item)
        results[name + '_centrifuga'] = {k2: v for k2, v in ctl('inventory').get('counts', {}).items() if 'bedrock' in k2}
        ctl('command', 'clear @s', check=False)
    evidence['etapa5'] = {'recolhido': got, 'feito': made, 'cristalizador': results}
    print('etapa5', json.dumps(evidence['etapa5'], ensure_ascii=False))


def nivel4():
    """A level 4 deposit (average density over 1,5: chunk (164, −117)):
    2 000 mB of solvent an extraction."""
    y = floor_y()
    x4, z4 = 2632, -1861  # the core lands 3 north: (2632, −1864)
    if not (game / 'bedrock-chao4').exists():
        tp(x4 + 0.5, y + 2, z4 + 6.5)
        time.sleep(4)
        ctl('fill', x4 - 8, y, z4 - 10, x4 + 8, y + 8, z4 + 8, 'minecraft:air')
        ctl('fill', x4 - 8, y - 1, z4 - 10, x4 + 8, y - 1, z4 + 8, 'minecraft:stone')
        (game / 'bedrock-chao4').write_text('1')
    ctl('command', 'clear @s', check=False)
    if not any('excavator' in b[3] for b in ctl('blocks', x4 - 4, y, z4 - 7, x4 + 4, y + 6, z4).get('blocks', [])):
        place(x4, y, z4, 'ntm:machine_excavator', (x4, y, z4 + 3))
    (cx, cy, cz) = find('ntm:machine_excavator', x4 - 4, y, z4 - 7, x4 + 4, y + 6, z4)
    ctl('fill', cx - 1, -63, cz - 1, cx + 1, cy - 9, cz + 1, 'minecraft:air')
    ctl('fill', cx, -64, cz, cx, -64, cz, 'ntm:ore_bedrock')
    ctl('fill', cx + 4, y, cz, cx + 4, cy, cz, 'minecraft:stone')
    ctl('fill', cx + 6, y, cz, cx + 6, cy, cz, 'minecraft:stone')
    fluid_barrel(cx + 4, cy + 1, cz, 'ntm:fluid_barrel_full__solvent', (cx + 6, cy + 1, cz))
    stand = (cx, y, cz + 6)
    open_at(cx, cy, cz + 3, stand)
    have = {sl[0] for sl in machine_slots()}
    if 300 not in have:
        put('ntm:battery_creative', 1, 300)
    if 304 not in have:
        put('ntm:drillbit__ferro', 1, 304)
    ctl('click-button', 'Broca')
    time.sleep(2)
    t0, raw = time.time(), []
    while time.time() - t0 < 900:
        time.sleep(15)
        open_at(cx, cy, cz + 3, stand)
        raw = [sl for sl in machine_slots() if sl[1] == 'ntm:bedrock_ore_base']
        if raw:
            break
    ctl('click-button', 'Broca')
    close()
    ex = machine_state(cx, cy, cz) or {}
    evidence['nivel4'] = {'nucleo': [cx, cy, cz], 'brutos': sum(sl[2] for sl in raw), 'segundos': round(time.time() - t0, 1),
                          'tanque': ex.get('tank')}
    print('nivel4', evidence['nivel4'])


def petroleo_d():
    y = oil_floor_c()
    ctl('command', 'clear @s', check=False)
    tp(X7 + 12.5, y, Z7 - 4.5)
    time.sleep(4)
    tank = lambda t: (t.get('fluid', ''), t.get('mb', 0))
    ev = {}
    # The fracking tower's legs reach 3 out: a patch north of the floor.
    if not (game / 'bedrock-chao7d').exists():
        ctl('fill', X7 + 8, y, Z7 - 24, X7 + 22, y + 12, Z7 - 12, 'minecraft:air')
        ctl('fill', X7 + 8, y - 1, Z7 - 24, X7 + 22, y - 1, Z7 - 12, 'minecraft:stone')
        (game / 'bedrock-chao7d').write_text('1')
    # (name, block, x, z, deposit, its y, the cell its screen opens from:
    # the cores are walled in, a leg or a side cell is clicked).
    for name, block, x, z, deposit, dy, cell in (
            ('poco', 'ntm:machine_well', X7 + 2, Z7 - 8, 'ntm:ore_oil', None, (-1, 1)),
            *([('pumpjack', 'ntm:machine_pumpjack', X7 + 8, Z7 - 8, 'ntm:ore_oil', None, (3, 1))] if 'sem-pumpjack' not in sys.argv else []),
            ('fraturamento', 'ntm:machine_fracking_tower', X7 + 15, Z7 - 18, 'ntm:ore_bedrock_oil', -63, (-3, 3))):
        if not any(block in b[3] for b in ctl('blocks', x - 2, y, z - 3, x + 2, y, z + 2).get('blocks', [])):
            place(x, y, z, block, (x, y, z + 3))
        core = find(block, x - 2, y, z - 3, x + 2, y, z + 2)
        # A second one a click left beside it (an earlier run).
        if ctl('block', core[0], y, core[2] + 1).get('state', '').startswith(block):
            ctl('fill', core[0], y, core[2] + 1, core[0], y, core[2] + 1, 'minecraft:air')
        dep = (core[0], dy if dy is not None else y - 12, core[2])
        if dy is not None:
            # Natural bedrock reaches −60: the pipe's way down cleared to it.
            ctl('fill', dep[0], dep[1] + 1, dep[2], dep[0], -58, dep[2], 'minecraft:stone')
        ctl('fill', *dep, *dep, deposit)
        # Empty-handed: a click with a block in the hand places it.
        ctl('command', 'clear @s', check=False)
        oc = (core[0] + cell[0], y, core[2] + cell[1])
        open_at(*oc, (oc[0], y, oc[2] + 2))
        have = {sl[0] for sl in machine_slots()}
        if 300 not in have:
            put('ntm:battery_creative', 1, 300)
        if 305 not in have:
            put('ntm:upgrade_overdrive_3', 1, 305)
        close()
        if name == 'poco':
            typed_barrel((core[0] + 1, y, core[2]), 'ntm:fluid_tank_full__oil', (core[0] + 3, y, core[2]))
        if name == 'fraturamento':
            fluid_barrel(core[0] - 1, y, core[2], 'ntm:fluid_barrel_full__fracksol', (core[0] - 3, y, core[2]))
        ev[name] = {'nucleo': core, 'deposito': dep}
    time.sleep(60)
    for name, e in ev.items():
        core, dep = e['nucleo'], e['deposito']
        m = machine_state(*core) or {}
        e['tanques'] = [tank(t) for t in m.get('tanks', [])]
        e['indicador'] = m.get('indicator')
        e['cano_acima_do_deposito'] = ctl('block', dep[0], dep[1] + 1, dep[2]).get('state', '')
        e['deposito_agora'] = ctl('block', *dep).get('state', '')
    ev['poco']['barril'] = barrel_state(ev['poco']['nucleo'][0] + 1, y, ev['poco']['nucleo'][2])
    evidence['petroleo_d'] = ev
    print('petroleo_d', json.dumps(ev, ensure_ascii=False))


def oil_machine(x, z, block):
    y = floor_y()
    if not any(block in b[3] for b in ctl('blocks', x - 2, y, z - 2, x + 2, y + 3, z + 2).get('blocks', [])):
        place(x, y, z, block, (x, y, z + 3))
    return find(block, x - 2, y, z - 2, x + 2, y + 3, z + 2)


def petroleo():
    y = stone_floor()
    ctl('command', 'clear @s', check=False)
    lq = oil_machine(X5 + 8, Z5 + 7, 'ntm:machine_liquefactor')
    if lq[2] != Z5 + 6:
        raise SystemExit(f'liquefator fora do lugar: {lq}')
    # The core lands a block off the click, toward the placer's back: the
    # same click offset puts the two side by side, their side ports touching.
    so = oil_machine(lq[0] - 3, lq[2] + 1, 'ntm:machine_solidifier')
    at = lambda c: (c[0], y, c[2] + 1, (c[0], y, c[2] + 3))
    open_at(*at(so))
    put('ntm:battery_creative', 1, 301)
    close()
    open_at(*at(lq))
    put('ntm:battery_creative', 1, 301)
    put('minecraft:coal', 4, 300)
    ctl('screenshot', game / 'petroleo-liquefator.png')
    close()
    t0, tar = time.time(), 0
    while time.time() - t0 < 90:
        time.sleep(6)
        open_at(*at(so))
        tar = sum(s[2] for s in machine_slots() if s[1] == 'ntm:oil_tar__coal')
        if tar >= 5:
            break
    ctl('screenshot', game / 'petroleo-solidificador.png')
    out = machine_slots()
    close()
    open_at(*at(lq))
    left = machine_slots()
    close()
    evidence['petroleo'] = {'liquefator': lq, 'solidificador': so, 'alcatrao': tar, 'solidificador_slots': out, 'liquefator_slots': left}
    print('petroleo', evidence['petroleo'])


XE, ZE = X7 + 80, Z7


def energy_floor():
    """A stone floor cleared 12 high east of the C floor (once)."""
    y = floor_y()
    if not (game / 'bedrock-chaoE').exists():
        tp(XE + 15.5, y + 2, ZE + 0.5)
        time.sleep(4)
        for x0 in range(XE - 6, XE + 40, 10):
            ctl('fill', x0, y, ZE - 12, x0 + 9, y + 12, ZE + 12, 'minecraft:air')
            ctl('fill', x0, y - 3, ZE - 12, x0 + 9, y - 1, ZE + 12, 'minecraft:stone')
        (game / 'bedrock-chaoE').write_text('1')
    return y


def click_nth(n):
    """The `n`th button of the open native screen (in its file's order;
    `screen` gives their rectangles, not their ids)."""
    bs = screen().get('buttons', [])
    if n >= len(bs):
        return False
    b = bs[n]
    ctl('click-at', b['x'] + b['w'] / 2, b['y'] + b['h'] / 2)
    time.sleep(0.8)
    return True


def heater_at(x, z, heater):
    """`heater` clicked at (x, z) on the floor: its core."""
    y = floor_y()
    if not any(heater in b[3] for b in ctl('blocks', x - 2, y, z - 4, x + 2, y + 1, z + 1).get('blocks', [])):
        place(x, y, z, heater, (x, y, z + 3))
    return find(heater, x - 2, y, z - 4, x + 2, y, z + 1)


def heater_with(x, z, heater, height, before=None):
    """`heater` clicked at (x, z) on the floor and a boiler on it, its core
    `height` above the heater's (placed from a pillar, sneaking): both
    cores. The boiler's offset is 1: its click column is the heater's core
    one toward the placer. `before(core)` runs before the boiler covers it."""
    y = floor_y()
    core = heater_at(x, z, heater)
    boil = (core[0], y + height, core[2])
    if before and not ctl('block', *boil).get('state', '').startswith('ntm:machine_boiler'):
        before(core)
    if not ctl('block', *boil).get('state', '').startswith('ntm:machine_boiler'):
        cx, cz = core[0], core[2] + 1
        pillar = (cx, y, cz + 2, cx, y + height + 1, cz + 2)
        ctl('fill', *pillar, 'minecraft:stone')
        close()
        tp(cx + 0.5, y + height + 2, cz + 2.5)
        give('ntm:machine_boiler', 1)
        ctl('key', 'shift', 'down')
        ctl('place', cx, y + height, cz, '--item', 'ntm:machine_boiler', check=False)
        ctl('key', 'shift', 'up')
        time.sleep(2)
        close()
        ctl('fill', *pillar, 'minecraft:air')
    if not ctl('block', *boil).get('state', '').startswith('ntm:machine_boiler'):
        raise SystemExit(f'a caldeira não ficou sobre {heater} {core}')
    # Its water from a barrel at its west side.
    barrel_up(boil[0] - 2, boil[1], boil[2], 'ntm:fluid_barrel_full__water')
    return core, boil


def energia_1():
    y = energy_floor()
    ctl('command', 'clear @s', check=False)
    tank = lambda t: (t.get('fluid', ''), t.get('mb', 0))
    ev = {}
    # The oil burner (2 high): heating oil in tanks in its input slot.
    ob, ob_boil = heater_with(XE, ZE, 'ntm:heater_oilburner', 2)
    ctl('command', 'clear @s', check=False)
    open_at(ob[0], y, ob[2] + 1, (ob[0], y, ob[2] + 3))
    # Only on the first run: a second click would put it out.
    slider = None
    if not any(sl[0] == 300 for sl in machine_slots()):
        put('ntm:fluid_tank_full__heatingoil', 16, 300)
        ctl('click-button', 'Ignition')
        time.sleep(0.8)
        # The slider's pixels 0..35 (ids 100..135) follow Ignition: pixel 32.
        slider = click_nth(1 + 32)
    ctl('screenshot', game / 'energia-queimador.png')
    close()
    # The electric heater: setting 4 on the old screen, which opens on the
    # core only (its top, seen from the plug, before the boiler covers it);
    # a battery at its plug (the clicked cell).
    def setting(el):
        old_screen(el[0], y, el[2], (el[0], y + 1, el[2] + 2))
        for _ in range(4):
            click_label('Trocar potência', 1.0)
        ev['eletrico_tela'] = [w.get('label') for w in ctl('ui').get('widgets', [])]
        ctl('screenshot', game / 'energia-eletrico.png')
        close()
    el, el_boil = heater_with(XE + 10, ZE, 'ntm:heater_electric', 1, setting)
    plug = (el[0], y, el[2] + 2)
    if not ctl('block', plug[0], y, plug[2] + 1).get('state', '').startswith('ntm:machine_battery'):
        battery(plug[0], y, plug[2] + 1, (plug[0], y, plug[2] + 4))
    # The heat exchanger: hot coolant from a barrel at its east corner.
    hx, hx_boil = heater_with(XE + 20, ZE, 'ntm:heater_heatex', 1)
    fluid_barrel(hx[0] + 2, y, hx[2] + 1, 'ntm:fluid_barrel_full__coolant_hot', (hx[0] + 4, y, hx[2] + 1))
    time.sleep(40)
    for name, core, boil in (('queimador', ob, ob_boil), ('eletrico', el, el_boil), ('trocador', hx, hx_boil)):
        h = machine_state(*core) or {}
        b = machine_state(*boil) or {}
        ev[name] = {'nucleo': core, 'aquecedor': h, 'caldeira': {'nucleo': boil, 'calor': b.get('heat'), 'entrada': tank(b.get('input', {})), 'saida': tank(b.get('output', {}))}}
    ev['controle_deslizante'] = slider
    evidence['energia_1'] = ev
    print('energia_1', json.dumps(ev, ensure_ascii=False))


for s in steps:
    globals()[s]()
evidence['took_too_long'] = 'took too long' in (game / 'client.log').read_text(errors='replace')
ev_path.write_text(json.dumps(evidence, indent=1, ensure_ascii=False))
print(json.dumps(evidence, ensure_ascii=False))
