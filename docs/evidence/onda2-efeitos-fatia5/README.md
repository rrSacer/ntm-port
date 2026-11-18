# D1+D2, retomada de 17/11: merges, dívida/drenagem direta e piloto no Stirling

## Base integrada

- Motor `onda2-writer-barrier`: merge `7f61a9c171` de `ntm-current` `2187c3192b` sobre a barreira `bb06eab65c`, sem
  conflito.
- NTM `onda2-efeitos`: merge `b2adf8cb` da `main` `fa3c1df3`. Só `horizon_revision` conflitou, em
  `native/ntm/compatibility.json` e `native/ntm/mixins/ntm/horizon-mixin.toml`; os dois apontam para `7f61a9c171`.
- No estado integrado, antes de qualquer mudança: 739 testes da biblioteca, Clippy `-D warnings` limpo e 134 testes em
  14 suítes de integração. O `save_model` em release passou 12/12: 8.361.560.872 sequências na profundidade 9, e a
  extensão R3 com 148.039.146 sequências na profundidade 10.

## Dívida/drenagem direta de um efeito `Items` (cenário D)

A prova de duas fases da fatia 4 nunca tinha alcançado a rota de dívida: a quebra da pilha segurava os itens atrás
de um efeito de blocos em B, e a aplicação acontecia no save. Para esta prova entrou o gancho de teste `ntmtest drop`
(só com `NTM_TEST_HOOKS=1`; commit `a5d1c4ba`). Ele tira a vareta de um canal RBMK como `WorldEffect::Items` do passo
do reator, sem efeito de blocos antes, para a célula (200, 101, 8). Essa célula fica no chunk (12, 0), longe de A,
fora da memória e sem vizinhança com chunks em ticking.

O controle vanilla usa barris nas mesmas células. O barril guarda a vareta enquanto B está ausente. Quando B entra, o
controle põe a vareta na mesma célula e esvazia o barril, uma vez.

Matriz estrita (`HORIZON_NO_EAGER_SAVE=1`), cliente `35df97c1…`, pacote `eebe930a…`
([owed-drain-matrix.json](owed-drain-matrix.json)):

| Corte | Dívida em A após o ato | Canais (NTM / controle) | Disco / 1ª / 2ª reabertura (NTM) | Controle | same_cut | Resultado |
| --- | --- | --- | --- | --- | --- | --- |
| none | pending=1 | 2 / 3 | 3 / 3 / 3 | 3 / 3 | true | passou |
| kill | pending=1 | 2 / 3 | 3 / 3 / 3 | 3 / 3 | true | passou (igual ao último save) |
| kv_committed | pending=1 | 2 / 3 | 3 / 3 / 3 | 3 / 3 | true | passou |
| save_begin | pending=1 | 2 / 3 | 3 / 3 / 3 | 3 / 3 | true | passou |
| region_record:1 | pending=1 | 2 / 3 | 3 / 3 / 3 | 3 / 3 | true | passou |
| chunks_written | pending=1 | 2 / 3 | 3 / 3 / 3 | 3 / 3 | true | passou |
| entities_written | pending=1 | 2 / 3 | 3 / 3 / 3 | 3 / 3 | true | passou |
| world_saved | pending=1 | 2 / 3 | 3 / 3 / 3 | 3 / 3 | true | passou |

Em todos os cortes: `lost=0`, sem duplicação, item a item igual ao controle, armazenamento limpo. Na reabertura, com B
ainda ausente, a dívida voltou do disco (`pending=1`). Com o jogador em B, ela foi paga ali uma vez, e na segunda
reabertura não havia nada devido nem pago de novo. O rastro do ato mostra um único `effect …0 items [200, 101, 8]`.
Evidência do ensaio: [trial-none/owed-drain/](trial-none/owed-drain/).

Limites:

- `Items` carrega id e contagem: o gancho perde o `custom_data` da vareta. A contagem é por id.
- Um `forceload` de chunk tão distante não trouxe a coluna para `world.chunks` neste ambiente; por isso o jogador vai
  até B, a 14 blocos da célula, para não recolher a vareta paga.
- A prova cobre `Items`. `EncodedItems` e `Entities` devidos seguem cobertos só pelos testes sintéticos da fatia 4.

## Piloto no Stirling (fatia 5)

Commits `0b3bfa7a` (núcleo) e `fd5053b0` (roteiro). Números, hashes e limites estão no doc do Stirling,
`docs/content-energy-solar-stirling.md`, seção "Piloto D1+D2, fatia 5". Resumo: 743 testes da biblioteca, Clippy
limpo, 8 mutantes mortos (4 novos e 4 da fatia 3), e o corte estrito de 7 pontos em 7/7, com 1 explosão no ato e 0 + 0
nas reaberturas ([stirling-overspeed-matrix.json](stirling-overspeed-matrix.json)). O dano ao jogador não foi
demonstrado: o centro da explosão fica dentro das células da própria máquina.

## Serraria: D3 e passos 1 a 4

Cada passo está descrito no artifact da onda 2 (seção 8). Estes arquivos são as suas provas; o binário de cada uma
está no próprio JSON (`binary_sha256`).

- [d3-step1-attempt1.json](d3-step1-attempt1.json): primeira sonda do passo 1, reprovada em `variant_drops_one`. A
  verificação achou dois itens no chão em vez de um: a variante sem lâmina e um item comum. O roteiro foi corrigido.
  Fica como histórico.
- [d3-step1-probe.json](d3-step1-probe.json): passo 1. O item com `{"ntm:no_blade":1b}` é lido na colocação e volta
  na quebra.
- [d3-step2-probe.json](d3-step2-probe.json): passo 2. A serraria sem lâmina guarda `has_blade=false` no modelo e
  continua sem lâmina depois de reabrir o mundo.
- [sawmill-game.json](sawmill-game.json): passo 3, uma sessão de jogador:
  - serrar um tronco em 6 tábuas;
  - a sobrevelocidade: 1 explosão e depois 1 lâmina, cada uma uma vez no rastreio;
  - o reparo à mão.
- `sawcut-trial-*.json`: os ensaios de corte `none` de cada cenário do passo 4.
  - `sawcut-trial-hand-attempt1.json`: o controle sem a lâmina instalada, corrigido depois.
- `sawcut-*-matrix.json`: as matrizes do passo 4 (`overspeed` 7/7, `border` 9/9, `break` 7/7, `hand` 8/8), contra
  barris vanilla.
- [sawcut-hopper-attempt/](sawcut-hopper-attempt/): o funil que alimentava a fornalha sugava a lâmina solta. Desde
  então ele sai antes da falha.
- [sawcut-x47-attempt/](sawcut-x47-attempt/): com o núcleo em x=47, a lâmina caía às vezes no chunk vizinho
  (`same_cut=false` com as mesmas contagens).

### Revisão independente: provas refeitas ([review/](review/))

Correção nos commits `0726064a` (núcleo e cola) e `17949c87` (ferramentas de prova). Cliente SHA-256 `0748e4ab…`,
pacote `eebe930a…`. Validação: 762 testes da biblioteca e Clippy limpo; 16 mutantes mortos (6 do grupo `review`, 3
`d3`, 3 `hand` e 4 `saw`).

- `d3-probe.json` passa; `sawmill-game.json` passa nas 9 verificações.
- `sawcut-break-matrix.json`: 7/7. O `disk_count.py` não conta mais a lâmina instalada de uma serraria em remoção
  pendente, porque o item da quebra já a carrega.
- `sawcut-overspeed-matrix.json`: 7/7.
- `sawcut-hand-matrix.json`: 8/8. Os resíduos vanilla são os mesmos de antes (−5 em `kv_committed`; 5 em
  `chunks_written` e `entities_written`), iguais aos do controle.
- `sawcut-border-matrix.json`: 8/9.
  - O `entities_written` deu `same_cut=false` com as mesmas contagens (1 e 1, sem perda nem cópia).
  - A lâmina do port saiu com o movimento aleatório de item solto e caiu no chunk (3,1), a leste da borda x=48. A do
    controle caiu no (2,0).
  - O núcleo do `border` passou para x=40, como o `overspeed`, e a matriz foi refeita com o passo 3c.

### Passo 3c: funis ([3c/](3c/))

Commits `783711b1` (núcleo e cola), `fdac1acc` (roteiros) e `47758d28` (contagem item a item). Cliente SHA-256
`0ab20b45…`, pacote `eebe930a…`. Validação: 765 testes da biblioteca e Clippy limpo; 25 mutantes mortos (9 novos do
grupo `hopper`, mais os grupos `review`, `hand`, `saw` e `d3` refeitos).

- `hopper-game.json`: 9/9 verificações.
  - Do funil de cima entra um tronco por vez, e o funil fica com 2.
  - O tronco é serrado em 6 tábuas e 1 de serragem.
  - Nada entra enquanto as saídas estão lá.
  - Um funil embaixo leva as saídas a um baú. Então o tronco seguinte entra e fica: a entrada não sai pelo funil.
  - Um tronco com nome, posto à mão, entra inteiro (`custom_name "Tora"` no slot, o `SlotCopy`).
- `sawmill-game.json`: 9/9, com os slots no BE (serrar, sobrevelocidade, reparo).
- `sawcut-hopper-matrix.json`: 7/7, com 8 átomos de madeira no disco, na 1ª e na 2ª reabertura, nos dois mundos.
  - A primeira tentativa, `sawcut-hopper-attempt1.json`, foi reprovada por um erro de roteiro. O total em jogo
    somava a lâmina instalada, que não é átomo deste cenário. A madeira já estava exata nos 7 cortes.
- `sawcut-hand-matrix.json`: 8/8, com os resíduos vanilla iguais aos do controle.
  - A primeira tentativa, `sawcut-hand-attempt1.json`, contou 10 de 11 no disco. O `disk_count.py` tomava o máximo
    entre o total dos slots e o total do modelo. Desde o 3c, o tronco fica no slot e a lâmina no modelo, então esse
    máximo perdia um dos dois. Agora conta item a item.
- `sawcut-border-matrix.json`: 9/9, com o núcleo em x=40 e `same_cut=true` em todos os pontos.

A segunda revisão reprovou este passo. As provas da seção abaixo, com o binário da correção, substituem as
destas pastas. Os cortes `overspeed` e `break` desta pasta e da `review/` foram feitos sem itens nos slots.

### Segunda revisão: provas refeitas ([review2/](review2/))

Commits, um por item:

- `3a9f670a` (P1: a restauração pelo journal adota os slots do BE);
- `81fb8bbb` (P2 residual: sai o ramo de marca atrasada);
- `5de819f2` (P3: a remoção de uma serraria do formato antigo);
- `91c336d2` e `bf0dd17b` (P3: o `SlotCopy` com nome);
- `80392695` (P3: a nota das portas);
- `8a371510` (P2-1: os cortes com os slots cheios);
- `d5e0f2b5` (os controles espelham a lâmina e recolhem todos os itens).

Cliente SHA-256 `062e13c5…`, pacote `eebe930a…`. Validação: 768 testes da biblioteca e Clippy limpo; 27 mutantes
mortos, entre eles os novos `RestoredSlotsOverwritten`, `WordToTheMachineThere` e `MigratingRemovalOnEngine`.

Refeito com este binário e validado:

- `d3-probe.json`: passa.
- `hopper-game.json`: 11/11, as 9 da primeira sessão e mais duas na segunda.
  - Depois de reabrir o mundo, o tronco nomeado continua no slot com o nome.
  - Com a serraria quebrada, o tronco que cai ainda tem `custom_name "Tora"`.
- `sawmill-game.json`: 9/9.
- `sawcut-overspeed-matrix.json`: 7/7. A lâmina quebra com as saídas no slot (6 tábuas e 1 serragem). São 8 átomos,
  contra um barril com a mesma lâmina e as mesmas saídas.
- `sawcut-break-matrix.json`: 7/7. Uma serraria sem lâmina com tronco, 6 tábuas e 1 serragem é quebrada, os itens
  são recolhidos e a serraria é posta de novo: 9 átomos.
  - Em `kv_committed` sobram 8 nos dois mundos, porque o arquivo do jogador é gravado antes das regiões.
  - Em `chunks_written` e `entities_written` faltam 8 nos dois mundos, porque a região é gravada antes do jogador.
  - São os mesmos resíduos vanilla do controle, item a item.
  - A primeira tentativa, `sawcut-break-attempt1.json`, ficou sem comparação em `kv_committed`. O `act` do controle
    falhou: o jogador só ia até o primeiro item caído, e o barril ficou fora de alcance.
- `sawcut-hopper-matrix.json`: 7/7.
- `sawcut-hand-matrix.json`: 8/8, com os mesmos resíduos vanilla do controle.
- `sawcut-border-matrix.json`: 9/9.
  - A primeira tentativa, `sawcut-border-attempt1.json`, deu `same_cut=false` em `world_saved` com as mesmas
    contagens. A lâmina do port escorregou em z para o chunk do núcleo.
  - Agora o controle põe a sua lâmina, parada, onde a do port caiu na mesma rodada.
  - Desde o `d5e0f2b5`, a posição da lâmina do controle no `overspeed` e no `border` não é mais independente. Ela
    vem do mundo do port na mesma rodada (`NTM_SAW_BLADE_AT`). O controle prova a conservação e o mesmo corte, mas não
    um lugar de queda próprio do vanilla.

Um tronco na entrada junto com saídas nos slots não é um estado alcançável jogando: tanto a mão quanto o funil
exigem os três slots vazios. Sob sobrevelocidade, um tronco é serrado muito antes de a lâmina quebrar, e o
resultado sobrescreve o slot 1, como no Java. Por isso o `overspeed` falha com as saídas, e é o `break`, sem calor,
que tem os três slots cheios.

Desde a primeira revisão, o `same_cut` de `none` e dos kills (`kill`, `kv_committed`) traz `same_cut_by_definition=true`:
esses pontos não cortam um save, então ali o `same_cut` não prova que os dois mundos pararam no mesmo limite.

## Escopo mantido

- `entities_written` de `coupled-break` segue **não aprovado** para `same_cut`. A diferença física de canal é real: a
  pilha ejeta em B e o baú solta em A. A conservação 6/6 está provada. `same_cut` não foi relaxado.
- Nenhum consumidor permanente foi ligado além do piloto do Stirling, que é a fatia 5 autorizada. O gancho `drop` só
  existe com `NTM_TEST_HOOKS=1`.
- Os 10 scripts protegidos estão intactos. Os mundos descartáveis foram removidos depois de salvar os relatórios.
