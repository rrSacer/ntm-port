# Portas, comportas e fechaduras K3: domínio puro (T6)

Frente T6 do port NTM → Horizon. Módulo: `crates/ntm-core/src/doors_security.rs`.
Testes: `crates/ntm-core/tests/doors_security_tests.rs` (34 testes, todos verdes).
Oráculo dos números: `tools/doors-oracle/extract.py` lê `DoorDecl.java` e `ModBlocks.java`
e gera `crates/ntm-core/tests/fixtures/doordecl.json`; `--check` falha se o JSON estiver defasado.

## Fontes Java (`ntm-src/src/main/java/com/hbm/`)

| Peça | Arquivo Java |
|---|---|
| 14 tipos de porta (`DoorSpec`) | `tileentity/DoorDecl.java` |
| Porta, `tick`, redstone, `tryToggle` | `blocks/generic/BlockDoorGeneric.java`, `tileentity/TileEntityDoorGeneric.java` |
| Porta blindada (6 blocos, vizinhas) | `blocks/machine/BlastDoor.java`, `tileentity/machine/TileEntityBlastDoor.java` |
| Selo e escotilha | `blocks/machine/BlockSeal.java`, `blocks/machine/BlockHatch.java`, `tileentity/machine/TileEntityHatch.java` |
| Trava, `canAccess`, pick | `tileentity/machine/TileEntityLockableBase.java` |
| Chaves e cadeados | `items/tool/ItemKeyPin.java`, `ItemKey.java`, `ItemLock.java`, `ItemCounterfeitKeys.java` |
| Buraco de fechadura | `blocks/generic/BlockKeyhole.java`, `BlockRedBrickKeyhole.java` |

## O que entrou

- **`spec(kind)`** com os 14 `DoorSpec`: tempo de abertura, faixas (`open_ranges`),
  dimensões, extra (só o cofre tem), offset, controle remoto, redstone.
- **`moving_cells`**: as células que a porta expõe (abrindo) ou recolhe (fechando)
  em cada tick, como `updateEntity`. Inclui `rotate_pos`, `block_rotation` e a
  tabela de `MultiblockHandlerXR.rotate` (`rotate_dims`).
- **`footprint`**: as células de uma caixa de `dimensions` (ou extra) em uma face.
- **`Door`**: estado (fechada / abrindo / aberta / fechando), `tick`, `try_toggle`
  (jogador), `toggle_passcode` (redstone e lógica), `update_redstone` (contagem
  com o `-1` da Java), `bomb_trigger` (só os remotos).
- **`Lock`**: `can_access` (pino igual, chave vermelha, tentativa de pick),
  `apply_padlock`, `counterfeit_keys` (2 chaves falsas com os mesmos pinos).
- **`BlastDoorGroup`**: estados, timer de 100 ticks com os passos de cada bloco
  (`Dummy`), abertura e fechamento de vizinhas, travamento que se propaga.
- **Selo**: `frame_size` (anel de 1 a 6), interior, `close`/`open`/`is_closed`,
  redstone com o bit 4 do `meta`, sobrevivência da escotilha.
- **Buraco de fechadura**: `keyhole_orientation` por lado.
- **Bomba**: `BlastDoorGroup::bomb_trigger` (destrancada: alterna, TRIGGERED;
  trancada: recusa) e `seal_explode` (com moldura: abre ou fecha; sem: nada).
- **Colocação da porta blindada**: `place_dummy` (`placeDummy`, com e sem `force`)
  e `blast_door_place` (`onBlockPlacedBy`: 6 dummies acima, para na primeira recusa,
  destrói o controlador, e os dummies já postos ficam).
- **Clique**: `player_can_click(larp, sneaking)`: agachado nunca chega à porta.

## Números conferidos porta por porta

O teste `every_door_matches_the_java_fixture` confere, para cada um dos 14 tipos
(via `fixtures/doordecl.json`, gerado do `DoorDecl.java` e do registro em
`ModBlocks.java`): id, `timeToOpen`, faixas, dimensões, extra, offset, controle
remoto, `neverDemandRedstone` e a janela de `getDoorRangeOpenTime` em todos os
ticks. A tabela escrita à mão foi removida: não há mais números digitados duas vezes.
A tabela abaixo é um resumo do fixture.

| Porta | Ticks | Dimensões | Offset | Remoto |
|---|---|---|---|---|
| transition_seal | 480 | 23,0,0,0,13,12 | 0 | não |
| vault_door | 120 | 4,0,0,0,2,2 (+ extra) | 0 | não |
| fire_door | 160 | 2,0,0,0,2,1 | 0 | não |
| sliding_blast_door | 24 | 3,0,0,0,3,3 | 0 | não |
| sliding_seal_door | 20 | 1,0,0,0,0,0 | 0 | não |
| secure_access_door | 120 | 4,0,0,0,2,2 | 0 | não |
| round_airlock_door | 60 | 3,0,0,0,2,1 | 0 | não |
| qe_sliding_door | 10 | 1,0,0,0,1,0 | 0 | não |
| qe_containment | 160 | 2,0,0,0,1,1 | 0 | não |
| water_door | 60 (janela 35–40) | 2,0,0,0,1,1 | 0 | não (nunca pede redstone) |
| silo_hatch | 60 (janela 20–20) | 0,0,2,2,2,2 | 2 | sim |
| silo_hatch_large | 60 (janela 20–20) | 0,0,3,3,3,3 | 3 | sim |
| large_vehicle_door | 60 | 5,0,0,0,3,3 | 0 | não |
| cargo_door | 60 | 2,0,0,0,1,1 | 0 | não |

Outros números com teste:

- Cofre: 25 células (5×5) na face; o extra é uma fileira de 5 à frente.
- Cofre aberto: 9 células expostas (o núcleo não conta).
- Water door: 8 células totais com a porta aberta; 5 aos 37 ticks.
- Silo: 9 células aos 19 ticks; 26 aos 20 e 21.
- Cofre: abre em exatamente 120 ticks e fecha em 120.

## Peculiaridades do Java mantidas (de propósito)

1. **Janela 20–20 do silo**: em exatamente 20 ticks a fração é `0/0 = NaN`, e a
   porta inteira abre de uma vez. Teste `silo_hatch_quirk_at_exactly_twenty_ticks`.
2. **`abs(len - 1)`** (e não `abs(len) - 1`) como divisor das faixas.
3. **`tryToggle` retorna `true` mesmo quando a trava recusa**: o clique é gasto.
4. **Porta blindada**: `(!travada && sensor_da_base) || sensor_do_topo`. Trancada, o
   sensor de baixo é ignorado, mas o de cima (6 blocos acima) ainda abre.
5. **Trava de vizinha**: o vizinho recebe o modificador antigo (`setMod` vem depois
   de `lock()` na Java). Teste `blast_lock_gives_neighbours_the_old_modifier`.
6. **`isSealClosed` devolve `true` quando há escotilha** (nome invertido da Java).

## Lacunas nominais (não portadas nesta frente)

- **Sala vermelha e porta vermelha** (`BlockKeyhole.generateRoom`,
  `BlockRedBrickKeyhole`, `ItemModDoor.placeDoorBlock`): o buraco de fechadura
  gera uma sala de tijolo vermelho e coloca `door_red`. Só a orientação por lado
  foi portada (`keyhole_orientation`).
- **`onScrew`** (troca de skin com a chave de fenda agachado, `getSkinCount`):
  skins são de aparência; não portado. `BlastDoor` e `Door` não têm skin no
  núcleo.
- **Verificação de espaço e preenchimento** do multibloco (`BlockDoorGeneric.checkRequirement`
  e `fillSpace`, que chamam `MultiblockHandlerXR.checkSpace`/`fillSpace`):
  `footprint` dá as células; a checagem de "há bloco ocupando" e o preenchimento
  com o bloco-fantasma dependem do mundo e ficam para o motor.
- **Orientação ao colocar** (`BlockSeal.onBlockPlacedBy`, `BlastDoor.onBlockPlacedBy`
  seta `meta` pelo yaw): não portado; o facing vem do chamador.
- **Dummies no mundo**: `place_dummy` e `remove_dummy` decidem, mas o bloco
  `dummy_block_blast` e o `TileEntityDummy` (alvo do núcleo) não existem no port.

- **Colisão dinâmica**: `getBlockBound` (a caixa de colisão de cada célula, aberta
  ou fechada) **não foi portado**. O motor aceita formas de colisão por bloco
  (`collision_shapes` em `horizon-client/src/mod_content.rs`), então um par
  fechado/aberto por bloco pode caber no template. Mas as células-fantasma que
  surgem e somem com o movimento dependem do servidor. **Não confirmado**: falta
  checar isso no template de cada um dos 16 IDs e no motor.
- **Brick forgotten lock** (`BlockForgottenLock`): só o som e o `return true` do
  clique foram lidos. A geometria que ele abre (`brick_forgotten`) não foi portada.
- **Sons, animações, skins, render** (`DoorDecl.getBusAnimation`, `getSEDNARenderer`,
  `getClippingPlanes`): do cliente, fora do escopo.
- **Itens com efeito de tooltip** (`ItemKeyPin.addInformation`, `canTransfer`):
  apresentação; ficam para a camada de itens.

## Oráculo e mutações

- `python3 tools/doors-oracle/extract.py --check` deve dizer "matches".
- Mutações feitas à mão nos números e nas regras, com os testes pegando cada uma,
  estão no relatório da frente.

## IDs cobertos e o que falta ligar (depois da Onda 0)

- 16 IDs `Kind::Inert` de porta e comporta: a lógica existe; falta o `registry.rs`
  ligar `Kind` e o tick.
- `seal_controller`, `stone_keyhole`, `stone_keyhole_meta`, `brick_forgotten_lock`,
  `key`, `key_red`, `key_kit`, `pin`: **não estão em `BLOCKS` do `registry.rs`**
  (só como itens/IDs de conteúdo). A lógica deles está pronta; o registro
  depende da frente de conteúdo.
- Ligar `Door::tick` ao servidor, `BlastDoorGroup::tick` com o `powered` real
  dos blocos (y e y+6), e a troca de blocos das células (`moving_cells` →
  `place`/`remove`) no `machine.rs`/`native_server.rs`.
- Persistir `Door`/`BlastDoorGroup` no save (`persist.rs`), depois da `wave0-vanilla`.
- Adicionar `pub mod doors_security;` já está feito em `lib.rs` (uma linha).
