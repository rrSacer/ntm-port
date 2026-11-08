# Rede elétrica avançada: domínio puro (T7)

Módulo `crates/ntm-core/src/energy_network.rs`. Não toca `energy.rs` nem o tick, o save ou os menus.
Prova: `tools/energy-net-oracle/` (Java real compilado, README próprio) e
`crates/ntm-core/tests/energy_network_tests.rs` (22 testes; doubles por bits).

## O que está portado

| Parte | Java de origem | Rust |
| --- | --- | --- |
| Distribuição | `PowerNetMK2.update()` | `PowerNet::update` + `EnergyEntity`: grupos de prioridade (do mais alto ao mais baixo), peso `double`, truncamento `long`, soma `long` com wrap, o desconto **acumulado** de `energyUsed` em `toTransfer` após cada grupo, débito proporcional dos provedores e compensação aleatória (até 100 sorteios, `NodeNet.rand`), `energyTracker`, remoção de entradas não carregadas |
| Sorteio | `java.util.Random` | `JavaRandom` (LCG de 48 bits; `nextInt(bound)` idêntico) |
| REDD | `TileEntityBatteryREDD` | `BatteryRedd`: `power`/`delta`/`log[20]` em `BigInt` (inteiro sem teto), janela `Long.MAX/100` (conexão) e metade dela (provisão), `transferPower`/`usePower` diretos, `setPower` nulo, slots 0 (entrada) e 1 (saída), velocidade do rotor, portas e conexões por orientação, `toByteArray`/`new BigInteger(byte[])` para o NBT |
| Soquete | `TileEntityBatterySocket` | `BatterySocket`: sem reserva própria, delega ao pack; velocidades por modo; `canExtractItem`; portas e conexões |
| Base de bateria | `TileEntityBatteryBase` | `BatteryBase`: `redLow`/`redHigh`, prioridade (LOWEST/HIGHEST voltam a LOW), modo por redstone, pertencimento à rede por modo, botões, comparador |
| Packs | `ItemBatteryPack` + `Library.charge*` | `PACKS`, `BatteryPack`, `ItemSlot` (NBT `charge`, pack sem tag, criativo), `charge_items_from_te`, `charge_te_from_items` |
| Fios | `TileEntityPylonBase` e 5 subclasses | `WireKind` (9 ids): tipo SINGLE/TRIPLE/QUAD, alcance 10/100/25/25/45×4/100, pontos de montagem (inclui `rotateAroundY` com a tabela de seno do Minecraft), `canConnect(dir)`, `createNode`; `can_connect_wires` (distância em `float`, como `Vec3.lengthVector`) |
| Chave/detector | `CableSwitch`, `CableDetector`, `TileEntityCableSwitch` | `switch_toggled`, `detector_meta_after`, `switch_joins_net`; migração de metadado do poste simples |
| 14 ids | `ModBlocks` | `NetBlock`, `NET_BLOCK_IDS`. `machine_transformer` é um bloco simples **sem** block entity no Java: não há comportamento a portar |

## Peculiaridades do Java preservadas (cada uma coberta por fixture)

- `toTransfer -= energyUsed` usa o `energyUsed` acumulado, não a parcela do grupo.
- `getPower` do REDD é `min(power, max/2)`; `getMaxPower` é `Long.MAX/100` (não é capacidade).
- Bateria criativa no slot 1 do REDD: `chargeItemsFromTE` devolve 0, então o REDD "remove" todo o `getPower()` a cada tick.
- Pack sem NBT ganha `charge=0` ao ser lido; `discharge` sem tag grava 0; `grabEnumSafely` usa `abs(i % 12)` (metadado 12 vira REDSTONE, negativo espelha).
- A velocidade do rotor vira `NaN` com energia negativa (`Math.min(NaN, 15)` é `NaN`).
- Distância do fio: `(float) sqrt(...)`; conector comum e super têm o mesmo tipo SINGLE e o limite é o **menor** dos dois alcances.

## Contagens da prova

Oráculo: 421 casos de rede (21 manuais, 400 aleatórios com até 40 nós; entre os manuais, 4 em que as somas de demanda/oferta estouram `long` e dão a volta, e 1 com 200 provedores de 1 HE que esgota as 100 sorteios da compensação; nós não carregados; REDD misturado), 70 sequências do REDD (10 ops cada), 612 combinações de soquete, 48 de botões, 144 estados de fios, 1.504 validações de link (4 na borda do arredondamento a `float`), 7 fluxos de `Random`, 25 `toByteArray`, 12 geometrias. Testes: 24 em `energy_network_tests`, e um deles garante que a fixture contém o estouro e o teto; mutações verificadas: trocar `wrapping_add` por `saturating_add` em `power_available`, `demand[p]`, `total_demand` ou `energy_used`, baixar o teto 100→99 e tirar o arredondamento a `float` da distância quebram os testes; os 452 testes de unidade e os de explosão do `ntm-core` seguem verdes; `clippy -D warnings` limpo.

## Lacunas nominais (não portado) e por quê

- **Grafo de nós/links (UNINOS / `Nodespace`)**: quais nós formam uma rede, `checkConnection`, fusão e divisão de redes, `recentlyChanged`. O ticket pedia a distribuição; o grafo existente do mod (`energy.rs`) é outro modelo e ficou intocado. Falta decidir se `energy.rs` adota estas regras ou se roda em paralelo.
- **Acessórios SC (`battery_sc`)**: multiplicador de flutuação `scPowerMult`, descarga em raio e dano. `getPower` do soquete ignora o multiplicador (item fora dos 12 packs).
- **Metadado de orientação de seis faces dos conectores** e sua colocação (`onBlockPlaced` devolve a face), pintura/cor, `disconnectAll`, item de fio (`ItemWiring`) que chama `canConnect` e `addConnection`.
- **Sinal de comparador/redstone de saída**, `markDirty` das portas quando o comparador muda, sincronização de rede (`networkPackNT`), GUIs, OpenComputers/ROR, som `fensuHum`, interface `ICopiable` (copiar/colar configurações).
- **Itens `IBatteryItem` fora do catálogo:** o Java aceita qualquer item que implemente a interface no soquete e no REDD (`cube_power`, bateria de batata, `memory`, `battery_sc`, núcleo de fusão, ferramentas e armaduras energizadas). `SlotItem` é fechado (12 packs + criativo + "outro"); ao entrarem no catálogo vira especificação de capacidade/taxas ou trait.
- **`updateEntity` do soquete:** a média móvel `log[20]`/`delta` (GUI e ROR) e o ramo do SC não foram portados; só o REDD tem `log`/`delta`.
- **`IPersistentNBT` do REDD:** `writeNBT/readNBT` (`power` e `muffled` ao recolher o bloco) não portado; `to_java_bytes` cobre o formato do `power`.
- **`isBadLink` / `isInvalid`:** o Java também descarta `TileEntity.isInvalid()`. O port só tem `is_loaded`; a integração deve mapear "bloco removido" para `is_loaded == false`.
- **Velocidade do rotor:** a comparação é do `float` final (70 sequências); `ln`/`powf` da libm podem diferir 1 ulp do `Math.log/pow` no `double` intermediário. Só animação.
- O Java usa `HashSet` (ordem por hash de identidade, aleatória): a ordem de provedores só afeta quem sofre a compensação aleatória (o total retirado não muda); o port usa ordem de inserção. Cuidado na integração: `remove` + `add` põe a entrada no fim da lista (o `HashSet` não faria isso) e o fio do tick deve manter uma ordem estável.

## O que falta ligar depois da Onda 0

1. `pub mod energy_network;` já está em `lib.rs` (1 linha); ligar ao tick do servidor: construir `PowerNet` por rede, chamar `update` com o sorteio global e `reset_trackers` por tick, usando `machine.rs`/`native_server.rs`.
2. Amostrar redstone nas portas (`port_positions`) e chamar `BatterySocket::update`/`BatteryRedd::update`.
3. Persistir no save: `BatteryRedd.power` via `to_java_bytes`, `red_low/red_high/priority/last_redstone`, a lista de links e a cor dos fios, o metadado da chave/detector.
4. Trocar os 14 `Kind::Inert` em `registry.rs` por tipos reais (`NetBlock`) e os 12 `battery_pack` por itens com NBT `charge`.
5. Decidir a relação com o grafo de `energy.rs` (lacuna 1).
