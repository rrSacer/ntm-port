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
