# Energia, lote 6: baterias e rede

O lote acrescenta 29 identidades e leva o catálogo a 2.800 IDs (446 blocos e 2.354 itens): 12 variantes metadata de `battery_pack`, quatro cabos simples e 13 blocos com estado reservado para a camada nativa. Os contratos foram combinados com a frente de lógica antes da geração.

| Grupo | IDs |
| --- | --- |
| Cabos simples, sem block entity | `red_cable_classic`, `red_cable_paintable`, `red_cable_gauge`, `red_cable_box` |
| Conectores de fio | `red_connector`, `red_connector_super` |
| Pylons | `red_pylon`, `red_pylon_steel`, `red_pylon_medium_wood`, `red_pylon_medium_wood_transformer`, `red_pylon_medium_steel`, `red_pylon_medium_steel_transformer`, `red_pylon_large` |
| Controles | `cable_switch`, `cable_detector` |
| Armazenamento | `machine_battery_socket`, `machine_battery_redd` |

Os quatro cabos usam `stone` como template simples. Os outros 13 blocos usam `beehive`, `facing` horizontal e `honey_level` 0..5, com `auto: false` no visual-gen; o responsável pela lógica os registra como Inert. Esses templates reservam o suporte a block entity para quando a lógica vier. `cable_diode` é Deprecated e oculto no criativo do fonte, ficando documentado como lacuna legada.

Os 12 packs preservam os ordinais de `EnumBatteryPack`, nomes e limites de carga/descarga. Baterias comuns armazenam descarga×18.000 ticks e carregam a dez vezes a descarga. Capacitores armazenam descarga×600 e carregam à mesma taxa de descarga. A bateria quântica usa 72.000 ticks: 72 bilhões de HE. O ícone de inventário usa a textura registrada original `battery_creative_new`; o renderer 3D por pack, carga NBT, tooltip e variantes criativas carregadas ficam para suporte nativo.

`machine-recipes/energy_storage.json` conserva os algoritmos e a procedência:

- O socket tem um slot e delega reserva e taxas ao `IBatteryItem` inserido. Não foi inventada uma reserva própria. O estado do item é o long NBT `charge`; setters do fonte não fazem clamp sozinhos. Acessórios SC e seus efeitos especiais ficam fora deste lote.
- O REDD armazena energia em BigInteger sem teto fixo. Sua janela de rede é `Long.MAX_VALUE/100` (92.233.720.368.547.758 HE), e a janela de provisão é metade disso. Esses valores limitam a conexão, não a capacidade de armazenamento. `transferPower` adiciona, `usePower` subtrai e `setPower` não opera. A construção original permanece na forja de plasma (`ass.fensusan`), ainda não portada.
- Modos de bateria: entrada, buffer, saída e desligada. Redstone em qualquer porta escolhe `redHigh` (padrão saída); sem sinal usa `redLow` (padrão entrada). Prioridade padrão LOW, botões e comparador preservados no contrato.
- O alcance dos fios é 10 blocos no conector comum, 100 no super, 25 no pylon comum, 45 nos médios e 100 no grande. A conexão exige tipos SINGLE/TRIPLE/QUAD compatíveis, endpoints distintos e distância menor ou igual ao menor alcance. Esses tipos não são limites de quantidade de links.
- `cable_switch` alterna por clique; `cable_detector` é controlado pelo sinal de redstone vizinho e usa o mesmo tile de chave. O detector não mede fluxo de energia. A pintura/disfarce, medidor, caixas, links, colisões, modelos e orientações de seis faces do original são lacunas explícitas.
- A distribuição MK2 conserva prioridade descendente, ponderação por demanda/oferta, truncamento long e compensação aleatória limitada a 100 iterações. Uma peculiaridade do fonte está registrada: após cada grupo de prioridade ele subtrai `energyUsed` acumulado de `toTransfer`, em vez de apenas a parcela desse grupo. Não se adicionou perda artificial de cabo nem limite HE/t sem fonte.

As receitas originais resolvidas incluem socket, cabos/conectores/pylons e packs: duas variantes de pack na bancada, cinco na montadora e cinco na planta química. O catálogo tem 960 receitas vanilla. Montadora: 448 operações; planta química: 59. A bateria quântica consome 8.000 mB de perfluorometil frio e devolve 8.000 mB aquecido. A bigorna chega a 119 construções com o pylon grande (tier mínimo 2, AnvilRecipes.java:475). As três tabelas Rust foram regeneradas pelo machine-gen com aprovação da frente de lógica; `machine_files.json` não mudou.

Hazards recebeu 29 entradas de taxa zero sem registro fonte resolvido. Nenhuma taxa nem outro hazard de item antigo mudou; radiation_buckets não mudou. Não há alegação de segurança física dos itens.

Solução reaproveitada: catálogo de energia, tradutores de receitas vanilla/máquinas, extrator de hazards, geradores de visual e tabelas Rust e o verificador de jogo existente. Este lote declara conteúdo e snapshots; não implementa comportamento de bateria ou rede.

Validação: 182 testes por seed 1/42 e os validadores de conteúdo/receitas passaram. O pacote passou no `cargo-horizon-mod check` do upstream e do fork. Uma exportação isolada reproduziu 14.625 arquivos de conteúdo/dados, e os 14.546 arquivos assets/data do pacote coincidem com o conteúdo preparado para commit. A prova em ntm-next `15d31b261` com bundle NTM/Recipes passou em 171 verificações: os 13 blocos nos estados 0/5, os quatro cabos e todos os 29 IDs conferidos no inventário. Detector de rejeição/panic limpo. A prova confirma conteúdo e fallback visual, sem afirmar funcionamento de máquinas Inert.

Evidências locais em `content-gen-evidence/energy-storage-network/`. Pacote persistente: `dist/ntm-content-energy6.hmod`, 6.744.614 bytes, SHA-256 `cbf941222166f97a0e71275e3555610a88b21a25cf6aa40506389e034ada6384`. O mundo descartável foi removido após preservar os logs; instalações reais não receberam o pacote.
