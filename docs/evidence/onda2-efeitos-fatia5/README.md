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

## Escopo mantido

- `entities_written` de `coupled-break` segue **não aprovado** para `same_cut`. A diferença física de canal é real: a
  pilha ejeta em B e o baú solta em A. A conservação 6/6 está provada. `same_cut` não foi relaxado.
- Nenhum consumidor permanente foi ligado além do piloto do Stirling, que é a fatia 5 autorizada. O gancho `drop` só
  existe com `NTM_TEST_HOOKS=1`.
- Os 10 scripts protegidos estão intactos. Os mundos descartáveis foram removidos depois de salvar os relatórios.
