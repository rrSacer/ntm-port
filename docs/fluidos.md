# Fluidos (Marco 3)

2026-10-29. Rust/WASM, sem mudar o motor. Fonte: `ntm-src` (NTM 1.7.10, versão
nova com `GenericRecipes`); caminhos de Java relativos a
`src/main/java/com/hbm/`, e `api/hbm/` para a rede.

| Peça | Onde |
| --- | --- |
| Tipos (157), tanque, recipientes, rede (`distribute`) | `crates/ntm-core/src/fluid.rs`, `data/fluids.json` |
| Extração dos tipos | `tools/fluid-gen/extract.py` (de `inventory/fluid/Fluids.java:261-417`) |
| Duto, barril e tanque como máquinas | `crates/ntm-core/src/fluidbox.rs` |
| A rede no servidor | `crates/ntm-core/src/server/fluid_server.rs` |
| Testes | `fluid::tests`, `fluidbox::tests`, `server/tests/fluid_tests.rs` |

## Tipos

Os 157 tipos do NTM saem do `Fluids.java` por script. O id é a ordem de registro:
NONE = 0, WATER = 1, UF6 = 29, AIR = 151, e TITANIUM_TETRACHLORIDE é forçado em
156, o que o extrator confere. Cada tipo guarda:

- o `stringId` em minúsculas (`uf6`), que é o nome nos saves, nas receitas e nos
  ids de item;
- cor e temperatura;
- os traços que importam aqui: gasoso, líquido, viscoso, corrosivo (nível),
  radiação de vazamento, antimatéria, `NOCON`, `LEADCON`, lata e botijão.

O content (`content/fluids.json`, do agente de importação) foi cruzado com
esta tabela.

## Tanque

Um tipo (ou nenhum ainda) e mB. `set_type` esvazia o tanque quando o tipo muda,
como `FluidTank.setTankType`. **Pressão (0–5 PU) fica de fora**: todo tanque
está a 0, e as receitas que exigem pressão foram puladas na extração.

## Recipientes

`FluidContainerRegistry.java:33-91`. O sufixo é o `stringId` do fluido:

| Recipiente | mB | Para quais fluidos |
| --- | ---: | --- |
| `fluid_tank_full__<f>` → `fluid_tank_empty` | 1.000 | sem `NOCON` e sem `LEADCON` |
| `fluid_tank_lead_full__<f>` → `fluid_tank_lead_empty` | 1.000 | sem `NOCON` |
| `fluid_barrel_full__<f>` → `fluid_barrel_empty` | 16.000 | sem `NOCON` e sem `LEADCON` |
| `canister_full__<f>` → `canister_empty` | 1.000 | quem tem `CD_Canister` |
| `gas_full__<f>` → `gas_empty` | 1.000 | quem tem `CD_Gastank` |
| `cell_uf6`, `cell_puf6`, `cell_deuterium`, `cell_tritium`, `cell_sas3` → `cell_empty` | 1.000 | — |
| `water_bucket`, `lava_bucket` → `bucket` | 1.000 | — |

Um teste confere os 472 recipientes despejáveis do `ids.json` nos dois
sentidos: o que o content tem existe aqui, e o que existe aqui está no content.
Os `fluid_pack_full__*` (32.000 mB) **não** são despejáveis: só a montadora
empacota e desempacota (`AssemblyMachineRecipes.java:1114-1121`).

Todo recipiente entra e sai por **transações de item**:

- despejar: `TakeHeld` do cheio; ao ser creditado, o fluido entra e o vazio
  volta por `Give`;
- encher: `TakeHeld` do vazio; ao ser creditado, sai o cheio.

Antes de pedir a transação, a máquina confere espaço ou fluido. Se o tanque
mudou nesse meio-tempo, o item volta como veio. O rollback por crash tira o
fluido de volta (`unconsume`).

## A rede (`FluidNetMK2`)

- **Duto** (`ntm:fluid_duct_neo`): é uma "máquina" sem nível, com um tipo de
  fluido escolhido na tela. Há uma lista paginada com os 156 fluidos, ou
  **Usar o fluido do recipiente na mão** (o NTM usa o identificador). Um duto
  sem tipo não liga nada, e só dutos do mesmo tipo se ligam entre si.
- **Portas:** toda máquina com tanques tem uma porta aberta nas 6 faces
  (`TilePortShapes.around`, num bloco só).
- **Topologia:** por dimensão e por tipo de fluido, o mesmo `Topology::build`
  da rede de energia, com os dutos daquele tipo no papel dos cabos. Ela é
  refeita quando muda a forma: dutos carregados e seus tipos, máquinas e os
  tipos dos tanques. Essa forma é conferida por hash a cada 10 ticks, então um
  duto posto ou retipado liga em até meio segundo.
- **Tick:** em cada rede, as ofertas e as demandas são lidas uma vez. Os
  receptores são servidos por prioridade (alta primeiro), cada prioridade em
  proporção à demanda, e os provedores drenam em proporção à oferta.
- **Save:** o fluido movido é salvo como energia e progresso (em até 200
  ticks). Quem move itens salva na hora.

## Barril e tanque

| Bloco | NTM | Capacidade | Vazão |
| --- | --- | ---: | --- |
| `ntm:barrel_steel` | `TileEntityBarrel` | 16.000 mB | sem limite |
| `ntm:machine_fluidtank` | `TileEntityMachineFluidTank` | 256.000 mB | `max(500, enchimento/100)` por tick |

- **Modos** (`TileEntityBarrel.java:78-90, 229-241`): entrada (só recebe),
  buffer (recebe e dá, com prioridade baixa), saída (só dá), desligado.
- **Tela:** a barra de enchimento, o modo, despejar, encher e "tipo pelo
  recipiente da mão".
- **Nível do bloco** (`honey_level`): mostra o enchimento.
- Ao quebrar, o fluido se perde. No NTM ele fica no NBT do item, e o mod não
  escreve NBT.

## Custo (fuel, harness `spikes/m2-fuel`, com pool)

| Operação | Fuel |
| --- | ---: |
| Topologia de uma linha de 400 dutos + 2 barris (só quando a rede muda) | 3,56 M |
| Um tick de `distribute` entre 50 barris de saída e 50 de entrada | 115 k |
| Um tick de `distribute` entre 5 e 5 | 14 k |

## Verificação

- **Mundo falso:**
  - um duto sem tipo não liga nada;
  - um duto de outro tipo bloqueia;
  - com o tipo certo, 9.000 mB passam de um barril para o outro;
  - o nível no bloco e o save conferem;
  - recipientes despejam e enchem por transação, e um recipiente sem fluido
    bastante é recusado antes de sair da mão.
- **Em jogo** (Xvfb, mudo, `tools/test-game/m3.py`):
  - 16 tanques de ar despejados num barril, posto em modo saída (`docs/img/m3-barrel.png`);
  - 2 dutos tipados "ar" pelo recipiente da mão;
  - a planta química, com a receita de oxigênio, recebeu o ar pela rede e fez
    1.000 mB de oxigênio em ~8 s;
  - um tanque de gás vazio saiu cheio de oxigênio (`docs/img/m3-chemical-fluids.png`).
