# Fatia 4: evidência final da barreira e cortes de persistência

Estado auditado em 16/11/2026. NTM `d79c5681`; motor/pin `bb06eab65c05202136701e73b835d1933cce4705`.
A execução original terminou em 12/11/2026; os PIDs citados pelo coordenador estavam encerrados na retomada.
Não se repetiram builds ou os cinco cortes já aprovados.

## Matriz original no pin final

O relatório bruto está em [coupled-break-final.json](coupled-break-final.json), o resumo em
[summary.json](summary.json). Todas as execuções usaram `HORIZON_NO_EAGER_SAVE=1` e o cliente release.
As três leituras são disco antes de reabrir, primeira reabertura e segunda reabertura.

| Corte | NTM | Vanilla | same_cut | Resultado |
| --- | --- | --- | --- | --- |
| none | 6 / 6 / 6 | 6 / 6 / 6 | true | passou |
| kill | 6 / 6 / 6 | 6 / 6 / 6 | true | passou |
| kv_committed | 6 / 6 / 6 | 6 / 6 / 6 | true | passou |
| save_begin | 6 / 6 / 6 | 6 / 6 / 6 | true | passou |
| chunks_written | 0 / 0 / 0 | 0 / 0 / 0 | true | passou: resíduo igual |
| entities_written | 6 / 6 / 6 | 6 / 6 / 6 | false | não emparelhado |

O último corte passa em todos os critérios de conservação, mas não na exigência de emparelhamento de chunks. O NTM
gravou as seis saídas em B=(3,0); o controle de baú gravou cinco em A=(2,0) e uma em B. O harness terminou com código 1.
A matriz original **não foi aprovada** e `same_cut` não foi relaxado.

A diferença corresponde ao comportamento de produção: `pile_dismantle` ejeta as varetas no fim de cada canal
(`pile_server.rs`), como `TileEntityPileCore.java:545–564` (`ejectAll` / `dropItem`). Um baú destruído no núcleo
solta seu estoque em A. O coordenador autorizou uma fixture temporária de dropper na borda de A apontado para B,
para investigar somente `entities_written`, mantendo os mesmos critérios do harness. A pré-validação dessa referência terminou sem alcançar a equivalência (ver abaixo).

A busca espacial da fixture original começava em x=-8, descarregando origem e destino ao mesmo tempo neste ambiente.
A cópia de teste começou em x=24 e encontrou a configuração em x=8 nos dois mundos; o patch é
[fixture-search.patch](fixture-search.patch). Nenhum dos dez scripts protegidos do usuário foi alterado.

## Mutação e verificações

O mutante `PreparedSnapshotUnprotected` compilou e foi morto pela asserção de consumo da marca da API real,
com `HORIZON_TEST_RESOURCES` definido. Log: [prepared-snapshot-mutant.log](prepared-snapshot-mutant.log).
O script restaurou a fonte e a árvore do motor estava limpa na retomada.

Já registrados em `WORLD_EFFECTS.md`: 591 unidades do núcleo, 11 cortes sintéticos, quatro testes específicos do
writer (um deles executado explicitamente com recursos reais), Clippy do motor e build release integrado.
A prova em jogo aqui é de varetas e dívida; não prova explosão genérica nem entidades arbitrárias.
Os consumidores novos permanecem desligados até a prova do replay físico.

## Identidade dos artefatos

- Cliente SHA-256: `64e8570bf4c634047d3aadbfccc42ab620de4a724a341bbbc674ee79ddbc89be`.
- Pacote SHA-256: `dca249694fb01df23c3fcd05f995eb844d4a2af4ea3954c22962579bd47f265e`.
- Cliente preservado em `.agent-tmp/onda2-efeitos/proof-bin/horizon-client`, fora dos targets.
- Pacote preservado em `.agent-tmp/onda2-efeitos/dist/ntm.hmod`.
- Targets próprios removidos em 16/11; mundos descartáveis removidos após preservar os relatórios.

## Pré-validação adicional do dropper (16/11)

O motor e o binário permaneceram idênticos. Só a cópia temporária da fixture foi alterada; as diferenças estão em
[native_crash_more.py.patch](native_crash_more.py.patch) e [run-crash-more.py.patch](run-crash-more.py.patch).
O vanilla foi executado primeiro, com uma guarda que impede executar o lado NTM se a referência não satisfizer
estoque, distribuição física e contagens. A comparação `same_cut` não foi modificada.

- Estoque do disco base: A=(2,0) contém 5; B=(3,0) contém 1.
- Após a transferência e antes dos pulsos, o dropper em A contém 5.
- Após até 15 pulsos, ele ainda contém 5; o teste `reference_source_paid` falhou antes do corte.
- B dá `execute if loaded=false`, mas a consulta de bloco dá `loaded=true`.
- O lado NTM e o corte `entities_written` não foram executados nessa tentativa, porque a referência falhou.

Dados: [dropper-vanilla-preflight.json](dropper-vanilla-preflight.json) e
[dropper-vanilla-act.json](dropper-vanilla-act.json). O código do motor em
`game/commands/execute.rs:1040–1048` define `execute if loaded` por status de ticking de entidades, coluna residente/lit
mais carga das entidades concluída. Logo essa condição da fixture não demonstra que a coluna deixou a memória como
exige o teste real de dívida em `ledger_add_here`. A ausência de uma emissão do dropper foi observada; não se conclui
só dela se o motivo é falta de ticking da origem, do destino ou outra condição do dispositivo.

**Veredito:** cinco cortes da matriz original passaram; `entities_written` continua sem par estrito. Os arquivos
completos demonstram conservação 6/6 como evidência parcial. A fixture de dropper não foi aceita como equivalente.
Além disso, o cenário original não prova que o host entrou na rota owed/drain, pois a suposta descarga foi inferida
por uma condição mais forte de ticking. A prova futura precisa observar ausência real da coluna pela mesma API da
cola, manter a origem acessível e estabelecer um controle vanilla com a mesma posição de saída e o mesmo corte.
Os consumidores de efeitos continuam desligados até a prova do contrato de replay físico.


## Residência real e proposta de prova limitada (16/11)

A sonda vanilla com o mesmo cliente não disparou efeitos NTM e distinguiu os dois critérios:

| Jogador x | A residente | B residente | A execute-loaded | B execute-loaded |
| --- | --- | --- | --- | --- |
| 24,5 | sim | sim | sim | sim |
| 8,5 | sim | sim | sim | não |
| −7,5 | sim | não | não | não |

Relatório: [residency-probe.json](residency-probe.json); sonda reproduzível:
[residency-probe.py](residency-probe.py). Assim A continua acessível pela consulta de coluna da cola enquanto B
está realmente ausente, mas o estado de ticking difere do usado pela fixture original. Isso não demonstra por si
só que um dispositivo vanilla deve funcionar nesse estado.

Proposta enviada ao coordenador, ainda não executada: comparar em duas fases a dívida NTM em A com estoque vanilla
que permanece no dropper A até B voltar. No primeiro corte ambos guardariam A=5 mais a entidade antiga B=1;
uma alteração irrelevante em A faria o registro de origem participar do mesmo save. Após reabrir e carregar B,
drenagem NTM e disparo vanilla consumiriam A e produziriam B=6, com outro corte nos mesmos arquivos. Essa prova
validaria estoque pendente, drenagem e ordenação de gravação; não equivaleria à paridade completa do instante da
quebra nem à prova de replay físico. Nenhum consumidor foi ativado e o corte original permanece não aprovado.


## Prova em duas fases, execução (16/11)

Autorizada pelo coordenador como validação de invariantes de dívida/drenagem, **não** como paridade da quebra direta.
Mesmo cliente e pacote (SHA-256 acima); só a cópia do roteiro em `probes/two-phase-test-game` mudou; zero alteração
de produção; consumidores desligados; scripts protegidos conferidos por SHA-256.

**Tentativa 1** ([two-phase-attempt1/](two-phase-attempt1/)): falhou no roteiro, não no jogo.
- Vanilla: fase 1 válida (A=5 no dropper, B=1). Na fase 2 o dropper de A, voltado para leste, mira o funil vazio da
  transferência em B, e o funil devolve cada disparo: o estoque fica em 5 em qualquer número de pulsos. Isso também
  explica a pré-validação do dropper acima ("ainda contém 5"), que não decorria só do ticking.
- NTM: o roteiro exigia `pending>=5` no registro em memória logo após a quebra; o registro ficou zerado.

**Correções da fixture** ([two-phase-attempt2/native_crash_more.py.patch](two-phase-attempt2/native_crash_more.py.patch)):
o funil vazio é removido (verificado vazio) antes dos disparos vanilla da fase 2; o NTM só registra 6 leituras do
registro e a dívida é julgada no disco (`phase1_debt_valid`, que já soma itens devidos e lotes do registro de A).

**Tentativa 2** ([two-phase-attempt2/](two-phase-attempt2/)), corte `entities_written`, `same_cut=true`:

| Leitura | Vanilla A / B | NTM A / B |
| --- | --- | --- |
| Disco no corte (fase 1) | 5 (estoque) / 1 | 0 / 6 (entidades) |
| Dívida no registro de A | não se aplica | 0 em memória (6 leituras) e 0 no disco |
| 1ª reabertura, após pagamento | 0 / 6 | 4 / 2 (6 no total) |
| Disco após a fase 2 | 0 / 6 | 4 / 2 (6 no total) |
| 2ª reabertura | 0 / 6 | 4 / 2 (6 no total) |

- Vanilla: as duas fases passaram (estoque preservado em A com B ausente; com B residente, 5 disparos, A consumido e
  B=6; o arquivo de entidades de B passou de 1 para 6; nenhuma duplicação nas duas reaberturas).
- NTM: **a premissa da fase 1 não se reproduziu.** Com a coluna B ausente pela consulta de bloco, a quebra do núcleo
  ejetou as 5 varetas como entidades em (48,5; 102,5; 13,5), dentro de B, com `Age=0` e movimento inicial. Elas nunca
  receberam tick e o save as gravou no arquivo de entidades de B. A rota de dívida e drenagem não foi exercitada
  ([entity-positions.json](two-phase-attempt2/entity-positions.json)).
- Conservação NTM: 6 de 6 no corte e nas duas reaberturas, com 5 Ra-226Be e 1 Zr, sem perda nem duplicação. Após
  reabrir, a física levou a pilha de 4 até x=47,85 (A), e por isso a verificação posicional `outputs_only_in_B` reprovou.
- Rota exata não determinada: `ledger_add_here` e `drop_items` testam residência de B antes de gerar a entidade; não
  se verificou qual caminho gerou as entidades nem por que B passou nesse teste. Referência Java:
  `TileEntityPileCore.dropItem` usa `spawnEntityInWorld`, que em 1.7.10 descarta a entidade num chunk inexistente.

**Veredito:** a prova em duas fases **não validou** os invariantes de dívida/drenagem do NTM, porque a dívida não
ocorreu. Ela comprova só conservação 6/6 no corte e após as reaberturas, além do controle vanilla em duas fases.
`entities_written` segue **não aprovado** para paridade da quebra direta. Os consumidores continuam desligados.


## Sonda da rota das varetas com B ausente (16/11)

Aprovada pelo coordenador, só em cópia de teste. Mesmo binário release; rastreio pelas variáveis de execução já
existentes `NTM_TRACE_EFFECTS=1` e `NTM_TRACE_PHASES=1`, sem build e sem alteração de produção. Corte `none`. A cópia
do roteiro passou a registrar seis leituras após a quebra: residência de B e da saída (48,102,13), varetas vivas no
raio de 150 e o tick. Evidência em [route-probe1/](route-probe1/) e [route-probe2/](route-probe2/).

**Fatos:**
- Rastreio: um único lote com `blocks 125` (a pilha volta a tijolo, inclusive as células x=48–49 que ficam em B) e
  cinco `items [48, 102, 13]` (4 Ra-226Be e 1 Zr), cada um aplicado uma só vez, sem nova tentativa. Em seguida,
  `storage written at tick 1284` (o save do fim do ato).
- Ticks 1133 a 1282, após a quebra: dívida 0, B e a saída ausentes, **nenhuma vareta no mundo vivo**.
- No disco (tentativa 2): as 5 varetas estão no arquivo de entidades de B, em (48,5; 102,5; 13,5), com `Age=0`.
  Após reabrir, a célula (49,102,10) de B é `ntm:pile_brick`.
- Os critérios de residência coincidem. `ledger_add_here`, `home_loaded`, `drop_items`/`loaded_state` e o `block` do
  controle consultam todos `world.chunks`. `ledger_store` grava a dívida no chunk dono carregado e em `LEDGERS`; se
  tivesse havido dívida, o `ntmledger` a mostraria.

**Conclusão parcial:** a dívida não foi registrada porque, no momento em que `ledger_add_here` rodou, B constava
como residente. A rota usada foi `spawn_items`; as entidades nasceram já dentro de B e foram gravadas com ele. A janela
de aplicação fica entre os ticks 1282 e 1284, ou seja, no save. Sem build não foi possível identificar o que torna B
acessível nesse instante. Candidatos: `world_saving` → `native_settle` processando a quebra adiada (A residente sem
ticking não roda o passo antes) com B reentrando por streaming; ou o gancho de descarregamento
(`retire_unloaded` → `native_chunks_changed` → `put_back`), que devolve a coluna a `world.chunks` enquanto os mods
são avisados. A ordem do lote (o efeito `blocks` precisa das células de B e segura os `items` que vêm depois) explica
por que nada saiu antes disso.

**Para fechar:** um log de tick e de `world.chunks` dentro de `flush_machine_effects`/`ledger_add_here`, numa cópia de
cola compilada à parte (exige build e target próprio), ou uma leitura do caminho de save do motor. Nada foi alterado
em produção; os consumidores seguem desligados.


## Caminho de save do motor: leitura estática (16/11)

Leitura sem build no pin integrado `bb06eab65c`, feita pelo coordenador e conferida aqui. Por decisão do coordenador,
não houve build instrumentado.

**Confirmado:** a aplicação na janela entre os ticks 1282 e 1284 acontece dentro do save.
`Game::save_game_with` (`game.rs:9126`) chama `native_world_saving(wait)` (`game.rs:9141`) antes de qualquer gravação.
Na cola, `world_saving` faz `feed_changes`, depois `Server::native_world_saving` → `native_settle`. Esse caminho roda
`removal_steps`/`flush_machine_effects` para a quebra que ainda não fora processada, já que A estava residente mas sem
ticking. Isso explica por que, durante as seis leituras, não havia dívida nem varetas.

**Corrigido:** o caminho não é `save_chunks_eagerly`/`save_eagerly_heard`. Essa função retorna antes de qualquer
trabalho quando `failpoint::no_eager_save()` é verdadeiro (`game.rs:9167–9172`, `save.rs:2234`), o que vale com
`HORIZON_NO_EAGER_SAVE=1` (todas as rodadas) e com failpoint armado. O caminho é o save explícito do roteiro.

**Em aberto:** `native_world_saving` é o primeiro passo de `save_game_with`. Ele roda antes de `streaming.save` e de
`save_level` (`saving.rs:290`), e nada antes dele devolve B a `world.chunks` (o `put_back` só existe em
`native_chunks_changed`). O código lido não mostra, portanto, por que `ledger_add_here` encontrou B residente nesse
instante. O fato medido continua o mesmo: as entidades nasceram em B (`Age=0`) e foram gravadas no arquivo de B pelo
`save_entities` desse save. Fechar esse ponto exigiria um log em build próprio, que o coordenador dispensou.

**Decisão do coordenador:** `entities_written` segue **não aprovado** para a quebra direta instantânea. No Java 1.7.10,
`spawnEntityInWorld` descartaria as varetas num chunk ausente; no Horizon elas foram aplicadas pelo settle do save e
gravadas em B. A conservação medida foi 6 de 6, sem perda nem duplicação. Os consumidores seguem desligados e não
houve alteração de produção.
