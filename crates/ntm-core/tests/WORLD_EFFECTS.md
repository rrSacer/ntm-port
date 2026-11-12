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
