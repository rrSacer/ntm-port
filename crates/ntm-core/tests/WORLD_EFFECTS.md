# Onda 2: efeitos comuns de máquina — fatias de núcleo

Base: `40a50730`. Escopo confirmado pelo coordenador: fatias 1–3 da seção 7 do artifact
`onda2-efeitos-mundo`, sem cola, consumidor Stirling ou prova em jogo nesta rodada.

## Fatia 1

`WorldEffect::Explosion` conserva dimensão, centro double, força float e flags Java de fogo/quebra.
O host padrão recusa; `Deferred` guarda em `critical`; uma recusa no flush mantém cursor e ordem.
A variante `PileExplosion` continua desserializando saves antigos sem mudar seu comportamento.

Verificado: 583 testes unitários passaram (3 ignorados herdados), clippy `--all-targets -- -D warnings` limpo.
Três mutantes compilados e mortos por asserções: `ExplosionAcknowledgedOnRefusal`, `ExplosionFlagsLost`,
`ExplosionRepeatedOnItemRetry`. Reproduzir, sempre via `agent-run` com target próprio:

```sh
python3 crates/ntm-core/tests/mutate_world_effects.py 1
cargo test --lib
cargo clippy --all-targets -- -D warnings
```

Formatação: o módulo novo passa em `rustfmt --check`. `cargo fmt --all -- --check` falha na base também
(`transaction.rs` original e `examples/explosion_big.rs`, entre outros); os arquivos existentes mantêm o estilo
local para não misturar uma reformatação de todo o núcleo às fatias.

Contexto Java do consumidor futuro (não ativado aqui): `TileEntityStirling.java:90-105` cria explosão de força 5,
sem fogo/quebra e uma `EntityCog`; `EntityCog.java:60-67` devolve um `ModItems.gear_large` com o metadado da entidade
ao interagir e só mata a entidade quando o inventário aceita. Portanto perder a engrenagem continua defeito nominal;
a explosão, sozinha, não conclui o Stirling nem a jogabilidade da família.

## Fatia 2

No replay, um efeito posterior `Items`, `EncodedItems` ou `Entities` já visto no chunk de destino prova a aplicação
ordenada dos anteriores. O cursor avança sem nova aplicação/recibo. Não se aplica a bateladas novas nem a IDs alheios.
O teste inclui cursor inicial 3, dois efeitos anteriores e as três variantes de destino.

Correção correlata autorizada pelo coordenador: `native_obligations_loaded` rastreia uma casa que chega com dívida,
mesmo sem marca de migração. Sem isso, pagar tudo no primeiro `native_settle` deixa a casa fora de `sync_obligations`
e o anexo antigo continua no chunk. A sonda exige uma gravação vazia e uma segunda reabertura sem nova explosão.

Extensão `common/world_effects_order_model.rs` do `save_model`: dois registros independentes, captura nos dois sentidos,
writer FIFO com coalescência, morte, recusa e destino descarregado. Cada tick chama o `Server` real; o controle do
recorte R3 é a saída vanilla já salva (um item). O verificador anterior continua cobrindo conservação vanilla geral;
a extensão prova especificamente que o replay não destrói essa saída. Não pretende modelar o algoritmo de dano nativo.
Na primeira rodada, profundidade 9: 17.544 estados distintos, 22.053.400 sequências, zero violações. O padrão da extensão
foi elevado a 10 para acompanhar o máximo da Onda 0. Reproduzir:

```sh
cargo test --release --test save_model saved_outputs_survive_an_older_homes_explosion_replay -- --nocapture
python3 crates/ntm-core/tests/mutate_world_effects.py 2
```

A entrega só é pronta após finalizar o mutante `ReplayWithoutOrderGuard` nos dois oráculos, o mutante
`LoadedHomeNotTracked`, os testes finais e clippy; os números finais são enviados com o hash ao coordenador.

### Limite bloqueante da integração (revisão P1 do coordenador)

**R3 não é seguro no host nativo atual.** O contrato do núcleo exige que o destino que publica itens ou entidades
registre o ID de efeito, junto de seu chunk; o host sintético do verificador cumpre esse contrato. A cola atual não:
`glue.rs:620-623` e `648-650` fazem spawn direto em destino carregado sem marcar o ID, enquanto `effect_seen`
(`1248-1252`) só consulta `ledger_knows`. O mutante `HostWithoutLoadedMarker` reproduz o caminho de `Items` em destino carregado e deve
ser morto pelo verificador: replay sobre a saída já salva (dano repetido, mesmo se o drop recriado disfarçar a contagem)
ou duplicação de saída sem novo dano. O oráculo verifica também esses dois casos, além da perda do item marcado.

Marcar e testar os IDs nos destinos carregados para `Items`, `EncodedItems` e `Entities` é **requisito bloqueante da fatia 4**,
antes de qualquer consumidor de explosão + drop. Conferir também o dreno de dívidas em `ledger_tick`: `take_owed`
e `take_owed_entities` marcam o registro da origem que devia, não necessariamente o chunk de destino que recebe.
O modelo não simula o débito e sua drenagem: `DestinationOut` apenas recusa o efeito. `EncodedItems`, `Entities` e o caminho de dívida e drenagem precisam de provas próprias na fatia 4. Esta rodada não altera a cola nem valida o algoritmo nativo de explosão em jogo. A fatia 2 é infraestrutura sob contrato.

## Fatia 3: efeitos do passo e segunda porta

`Logic::world_effects` drena saídas runtime depois do `tick`, dentro do `Deferred`; o commit confirmado publica
modelo e fila crítica juntos. Sem consumidor de produção nesta rodada: o `String` inerte só tem uma sonda em
`cfg(test)` que consome um estado armado e produz uma explosão e um drop.

A convenção aprovada dos offsets é `[direita, cima, frente]`. Frente vem de `facing`, padrão `north`; direita é
leste quando a frente é norte. Drop parte da célula do núcleo. Explosão parte de `[x+0.5,y,z+0.5]`: offset
`[0,1,0]` dá literalmente `[x+0.5,y+1,z+0.5]` do Java. Testes cobrem north/default e east e outra máquina em outro chunk.

### Segunda porta: confirmação explícita da fase

O coordenador aprovou alterar a proposta inicial de Vec direto para:

```rust
server.publish_machine_effects(host, &key, |candidate| {
    // Só altera a cópia candidate e retorna as saídas locais do passo.
    Vec::<ntm_core::machine::MachineEffect>::new()
});
```

Ela recusa origem descarregada, removida, substituída ou de geração antiga, roda a fase numa cópia e confirma
`MachineCommit` completo (slots, modelo e dados). Só com aceitação troca o estado vivo, registra o sobrevivente e
põe a batelada em `machine_effects`, com `owners=[key]` e origem na geração observada. Recusa não altera nada e
permite repetir. A chamada fica no mesmo tick da fase, antes de `native_save`/`sync_obligations`.

O Vec direto não bastava: o finish futuro do Stirling vem depois de `local_steps`, logo `native_save` seria o primeiro
commit de `has_cog=false` e poderia recusar, com a obrigação já publicada. A closure torna esse primeiro commit parte
da porta; depois dele uma recusa de `native_save` é redundante e não pode deixar a batelada com o BE anterior.
O consumidor Stirling permanece desativado; mover seu finish para a closure cabe à fatia 5.

Oráculos preparados: recusa do passo comum sem estado/batelada, aceitação e uma única batelada na casa do núcleo;
recusa do primeiro commit pós-fase, repetição aceita, recusa posterior de save; captura entre este tick e o próximo,
duas reaberturas com a explosão recusada e uma aplicação quando aceita. Mutantes da fatia 3:
`PublishBeforeCommit`, `CommitWithoutBatch`, `PostPhaseWithoutCommit`, `PublishNextTick` (fila real drenada só no tick
seguinte). Reproduzir com `python3 crates/ntm-core/tests/mutate_world_effects.py 3` via `agent-run`.

As provas finais e números são enviados ao coordenador após rodar; ter código e oráculos preparados não é conclusão.
O bloqueio da marca de destino da fatia 4 continua valendo para consumidores que combinarem explosão e item/entidade.

## Evidências da rodada de núcleo (11/11/2026)

Código verificado no HEAD `f11dde1d`; os commits posteriores desta seção são somente documentação. Execuções por
`agent-run`, `QUEUE=cargo`, `CARGO_BUILD_JOBS=10`, `SKIP_UPSTREAM_CHECK=1`, com target próprio no SSD.

- Unidades do núcleo: **591 passaram**, 3 ignorados herdados, nenhuma falha.
- Integração fora do `save_model`: **118 passaram** em 12 suítes. Um primeiro comando de shell falhou antes de iniciar
  testes por expansão de variáveis do `systemd-run`; a execução refeita com argumentos estruturados passou integralmente.
- `cargo clippy --all-targets -- -D warnings`: passou. Uma execução anterior apontou clone desnecessário no teste;
  corrigido em `f11dde1d` e Clippy refeito sem isenções.
- Verificador `save_model` release completo antes da fatia 3, cujo modelo antigo permaneceu sem mudanças: **12/12**.
  Caso principal depth 9: **188.661.834** estados visitados, **12.399.394** distintos, **8.361.560.872** sequências;
  migração depth 8: **696.858.131** sequências para remoção e **691.838.579** para lote; 60.000 passos aleatórios.
  A extensão nova nessa rodada ainda era a versão anterior da prova R3, depth 9.
- Extensão R3 atual com `Server` real, rodando isolada em release no código final: depth **10**, **41.797** estados
  distintos, **148.039.146** sequências. Essa prova exige o contrato de marcador de destino e não valida a cola atual.
- Mutantes compilados e mortos por asserções: fatia 1 **3/3**; fatia 2 **3/3**, incluindo `ReplayWithoutOrderGuard`
  morto tanto por teste unitário quanto pelo verificador, e `HostWithoutLoadedMarker` morto pelo verificador; fatia 3
  **4/4**, incluindo `PublishNextTick` com fila realmente drenada apenas no tick seguinte. Erros de compilação e
  timeouts não contam como mutante morto. Logs são mantidos pelo Traycer (SHA-256 abaixo).
- `rustfmt --check` passou para os três módulos novos; `git diff --check` passou. `cargo fmt --all -- --check` continua
  falhando em arquivos preexistentes, como registrado na fatia 1. Nenhuma formatação massiva foi aplicada.
- Não houve prova em jogo nem corte contra baú nesta rodada, conforme a delimitação explícita das fatias 1–3. A prova
  em jogo com corte e o conserto dos marcadores são bloqueantes antes do piloto, nas fatias 4–5.

| Log do Traycer | SHA-256 | Conteúdo |
| --- | --- | --- |
| `13061341-0626-4d25-aa61-a1fa70cd7201/output.log` | `2b00ac277c61ffb6e63b09657a1c59267cf6ff9c841e94afbb1efc62a6d5be60` | checker completo anterior, mutantes da 2; termina com o primeiro Clippy recusado |
| `a7696b27-843c-49f9-90b6-3e3916a4af3f/output.log` | `eb9b2f7544e3387675550885697eab1e13496a60c3b4ffecc53cf0715ee96d60` | 591 unidades, R3 depth 10, mutantes da 3 e Clippy limpo |
| `f48db00d-9643-43fa-baee-d60b50f22c25/output.log` | `afd78e2c943537264eab27a51ffce5c603da7f9ed8e4bfce64ddbc33730d6b95` | erro inicial do comando e segunda execução, com 118 integrações verdes |

Os logs de mutação imprimem a falha de asserção esperada de cada variante; esses erros pertencem aos mutantes, não ao
código nominal. A conclusão de infraestrutura não autoriza o consumidor enquanto a marca de destino da cola faltar.

## Investigação da fatia 4 — marcadores e arquivos independentes

Sondas em `destination_marker_cut.rs` (4/4, Clippy `-D warnings`): usam o `Server` real e hosts sintéticos para
examinar candidatos, não são prova da cola nativa nem de um desenho completo. A casa A tem uma batelada antiga com
explosão e item; B é o chunk vizinho. O controle vanilla conserva 1 no baú/dropper correspondente de A.

- A antigo, região B com marcador novo, arquivo de entidades B antigo/vazio: suprimir item pelo marcador dá NTM 0,
  vanilla 1. A guarda também evita a explosão (correto para o efeito físico).
- A antigo, região **e** entidades B novas, com 1 item em B: suprimir item pelo marcador dá NTM 1, vanilla 2 (o
  item que a origem antiga volta a ter mais o salvo em B). O coordenador confirmou formalmente este análogo vanilla.
- Uma casa antiga que deve um item e um marcador B idêntico podem coexistir com entidades B antigas ou novas.
  Reemitir a dívida antiga preserva a cópia da origem em ambos os cortes; suprimir pelo marcador perde essa cópia.
  O teste anterior desta investigação usava como controle fixo 1 e errou ao considerar 2 uma duplicação indevida.
- No recorte limitado dos dois casos de A antigo e marcador B novo, usar o marcador para pular **somente a explosão**
  e pagar novamente o item da origem produz 1 ou 2, igual ao vanilla. É candidato parcial, não valida unload, ordem de
  entidade antes de região, coleta, merge, movimentação nem Entities/EncodedItems.

No pin `eb5d3eed58`, `save_level` agenda regiões antes das entidades e a passada eager grava só região; ambos têm
arquivos separados. Na cola há `EFFECT_TYPE` persistente legado e leitura dos anexos, mas o `spawn_item_own` do motor
retorna velocidade, não ID; a cola vê `next_entity` e UUID. Uma tag de entidade precisa ainda tratar remoção, coleta,
merge e deslocamento e não prova sozinha que os dados de região/entidade representam o mesmo corte. Nenhuma alteração
no motor ou no host foi feita nesta investigação. Os casos adicionais, o modelo de paridade e a prova com controle em
jogo são obrigatórios antes de aprovar a fatia 4.

Um quinto contracaso (`the_current_order_guard_also_suppresses_an_earlier_owed_item`, 5/5 no total, Clippy limpo)
usa a guarda atual do `Server` com um host sintético que reemite o devido de casa antiga. Na batelada
`[Items(ferro), Items(lâmina)]`, com o efeito posterior marcado, a guarda pula também o ferro: depois do replay, NTM 2
contra vanilla 3 (os dois itens da origem antiga e a lâmina no arquivo de entidades novo). A correção aprovada pelo
coordenador deverá restringir a guarda de efeito físico, sem suprimir o item devido; esta sonda demonstra o defeito
atual com o host candidato, não afirma que a cola já grave a marca em B.

Sexta sonda, `a_tag_lost_on_pickup_leaves_no_destination_evidence_to_stop_a_second_blast`: reconstrói A antigo com
batelada, B antigo sem marca ou entidade e arquivo do jogador novo com o item já coletado. Nesse host sintético, o
`Server` real reemite o item da origem (total 2, como o controle) mas também volta a chamar a explosão. O caso é um
modelo do corte; não foi produzido em jogo. Uma tag apenas na entidade sumiu ao entrar no inventário: no pin,
`item_entities.rs:249-266` passa `item.stack.clone()` para `give`, e `native_item_picked_up_v2` recebe stack,
thrower e posição, sem o ID da entidade. O mesmo estado de arquivos pode ter um item preexistente no jogador e uma
explosão ainda devida. A sonda delimita o alcance da proposta de tag; não prova que toda solução sem motor seja
impossível. O coordenador descartou flush síncrono extra. Seis sondas passaram; Clippy do teste limpo.

As sondas agora distinguem também arquivo de entidades novo com região B antiga: uma entidade ainda tagueada em B
permite ao host sintético provar a explosão anterior; quando o drop se funde a uma entidade antiga sem tag e apenas o
arquivo de entidades é gravado, o replay torna a explodir e pode perder as duas unidades salvas contra vanilla.
Se o drop foi salvo após mover-se ao chunk C, procurar a tag somente em B também não prova o efeito; o item distante
sobrevive, mas a explosão física se repete. As três sondas constroem os registros representativos em memória e usam o
`Server` real; **não** simulam ainda as transições de save do motor nem substituem o verificador exaustivo ou corte em
jogo. `cargo test --test destination_marker_cut` e Clippy desse teste: 9/9 e sem avisos. Naquela execução, a guarda
de produção ainda era a da fatia 2; a restrição a efeitos físicos foi implementada na rodada posterior.

A matriz `tag_and_marker_cut_matrix_covers_pickup_merge_and_movement` percorre 4 histórias (parado, coletado, fundido,
movido) × 16 combinações antigo/novo dos registros de B, entidades B/C e jogador, sempre com A antigo. Cada caso chama
`Server::native_settle` com host candidato que consulta marcador ou tag **em B** e reemite a dívida antiga. Resultado:
64 casos, 4 perdas e 12 explosões repetidas sobre saída que algum arquivo conserva; 10/10 testes de diagnóstico e
Clippy passaram. É uma **sobreaproximação**: não prova que cada combinação seja alcançável pela ordem FIFO do motor,
nem equivale a uma prova em jogo. Ela documenta onde o candidato simples falha e não declara a fatia 4 pronta.

### Invariante de origem e coalescência do writer

O coordenador aprovou explorar uma barreira **assíncrona** da origem, sem flush no tick: a cópia durável de A com o
efeito consumido deveria preceder qualquer arquivo de terceiros que possa conter sua emissão. No pin, enfileirar A1
antes de E_B não basta. `save.rs:3159-3171` consulta o snapshot pendente mais novo por alvo e pode pular A1 se A2 foi
encaminhado depois de E_B; `world_update.rs:180-198` enfileira entidades antes da região no unload. Assim o writer pode
pular A1, gravar E_B e cair antes de A2, deixando A antigo e E_B novo. Além disso, `save.rs:3175-3191` guarda um erro de
I/O mas continua o laço: falhar A1 não impede, por si, que E_B seja escrito depois.

`destination_fifo_witness.rs` transcreve essas regras no modelo e cobre os dois contracasos. Um **candidato de modelo**
marca A1 como obrigatório (não coalescível) e bloqueia escritas posteriores se A1 falha; com A1 inicialmente
encaminhado, explora todas as intercalações modeladas de enfileirar E_B e A2, gravar e falhar A1: **12 estados
únicos, zero violações** da precedência. Dois mutantes internos do modelo — coalescer A1, ou ignorar falha de A1 — são
pegos. `cargo test --test destination_fifo_witness`: 4/4; Clippy desse teste limpo após remover um argumento excedente.

Isto é um modelo limitado, não patch do motor, nem prova da cola ou de paridade vanilla integral: não cobre múltiplas
dimensões/writers, vários donos, falhas posteriores, persistência separada de jogador, descarregamentos e reaberturas,
nem confirma que a cópia de A contém o BE e os débitos/recibos certos. Uma implementação precisaria manter os dados
pendentes e retornar o erro a `flush`/`writes_finished` sem deadlock se o writer bloquear, e cobrir esses casos antes de
ativar um consumidor. Nenhum arquivo do motor foi alterado.

Um quinto caso do modelo do writer, `an_extra_required_source_write_can_change_the_same_unload_cut_against_vanilla`,
mostra que acrescentar uma gravação de A antes da descarga B pode mudar o estado no mesmo evento temporal: candidato
com A novo + E_B novo contém 1, controle com A antigo + E_B novo contém 2. O coordenador esclareceu que o harness
oficial emparelha cortes pelos chunks realmente gravados, não por relógio; portanto este caso **não é um veredito de
falha do critério oficial**. A primitiva de barreira deve agir só quando A já participa do ciclo normal de gravação;
não há autorização para salvar A artificialmente. Unload de B sozinho não enfileira A no pin (`world_update.rs:175-198`),
então a primitiva isolada também não fecha esse caso. As sondas do writer passaram 5/5 e Clippy do teste ficou limpo.

O protótipo isolado do motor, branch `onda2-writer-barrier` no pin `eb5d3eed58`, commit `5c3e305e62`, implementa
uma batelada de origem obrigatória, assíncrona e não coalescível. Se ela falha, os lotes posteriores dessa mesma
instância do writer não chegam ao disco, continuam pendentes e a falha é retornada; o caminho de save comum não usa a
barreira. No writer real, 77 testes de `save::tests` passaram, Clippy `-D warnings` passou e dois mutantes foram
mortos por asserções: coalescer a origem e ignorar sua falha. O primeiro teste foi ajustado para bloquear o writer
antes de avaliar a coalescência, depois que a versão inicial deixou o mutante sobreviver por uma corrida de teste.

Essa primitiva só ordena bateladas que o chamador efetivamente enfileirou na mesma `SaveDir`: não grava A por conta
própria, não coordena dimensões ou arquivos de jogador e não prova o conteúdo do snapshot ou a paridade em jogo. Não
foi integrada ao motor principal nem ao mod. A cola, o caminho owed/drain, a guarda de explosão e a prova em jogo
permanecem pendentes antes de habilitar consumidores da fatia 4.

### Integração em andamento (12/11)

O replay passou a aplicar `later_seen` apenas a blocos e explosões (`17464373`), deixando itens e entidades da origem
antiga devidos. O mutante que suprimiu os itens novamente foi morto pelo corte com duas saídas. Foram aprovados 591
testes de unidade (3 ignorados), os 11 cortes sintéticos `destination_marker_cut` e Clippy do núcleo. O corte de
`owed/drain` (`9e659b8e`) distingue itens, SNBT codificado e entidades em registros antigo/novo independentes; é um
modelo, não a prova do host.

A cola e o pin estão em `4a2e9080`: explosão genérica escolhe `Keep` ou `Block`, e a gravação do ledger marca a
origem para proteger a próxima gravação viva naturalmente agendada. Motor `95273a4133`, sobre `5c3e305e62`: três
testes do writer, Clippy e dois mutantes específicos da marca passaram. O build nativo release e o Clippy da árvore
transformada passaram; não se mudou a ordem de unload. A marca não força save e não é consumida por snapshot não vivo.

Na primeira tentativa de `coupled-break`, o roteiro original começou a busca em x=-8, com origem e destino já
descarregados. Uma cópia temporária iniciou a busca em x=24 e encontrou a configuração exigida em x=8 em ambos os
mundos. O corte sem interrupção passou com 6 itens no NTM e no vanilla, no disco e nas duas reaberturas. O corte
`kill` em modo eager deu NTM 5 e vanilla 0; o próprio harness classifica o modo eager como `proof=false`, portanto não
é prova do contrato. A execução em modo estrito está em andamento. Persistem a prova de replay físico e os cortes em
jogo de explosão/entidade: nenhum consumidor novo deve ser ativado antes deles.

Auditoria posterior do pin intermediário `95273a4133`: `save_chunks_eagerly` e saída também usam `save_later` em
snapshots vivos preparados com BEs, e o unload usa o mesmo método depois das entidades. Assim, a distinção por
`live_save` era insuficiente; o pin intermediário **não** é a integração final. Uma nova entrada para snapshots
preparados cobre esses três caminhos sem mudar a flag de failpoint nem a ordem do unload; a geração segue isolada.
As provas no pin intermediário, incluindo `none` e `kill` em modo estrito com 6/6, não validam o novo pin. No corte
`kv_committed` o controle não abriu porque o caminho do socket excedeu 108 bytes; não é resultado de paridade.


### Retomada auditada (16/11)

A matriz final do pin `bb06eab65c` já havia terminado: não se repetiu o build. Cinco cortes estritos passaram
(`none`, `kill`, `kv_committed`, `save_begin`: 6/6; `chunks_written`: 0/0, resíduo igual). O corte
`entities_written` conserva 6 itens no NTM e no vanilla no disco e nas duas reaberturas, mas permanece reprovado por
`same_cut=false`: NTM concentra as saídas em B, o baú vanilla solta cinco em A e uma em B. A saída do canal NTM segue
`TileEntityPileCore.java:545–564`; não se alterou produção para mascarar a diferença.

Uma pré-validação autorizada com dropper vanilla em A apontado para B não conseguiu pagar as cinco varetas e foi
interrompida antes do corte e do lado NTM. Ela mostrou também que `execute if loaded=false` não prova descarga da
coluna: B continuava residente (`control block` dava `loaded=true`). Portanto o cenário original não é prova de
entrada na rota owed/drain da cola. O mutante `PreparedSnapshotUnprotected` foi morto por asserção; fonte restaurada.
Relatórios, patches da fixture e limites estão em [evidência da fatia 4](../../../docs/evidence/onda2-efeitos-fatia4/README.md).
Não declarar a matriz ou o replay físico aprovados. Os consumidores novos permanecem desligados.
