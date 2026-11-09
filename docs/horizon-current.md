# Horizon atual e bombas otimizadas

Decisão do usuário em 01/11/2026: trabalhar com o Horizon oficial mais recente,
adaptando o NTM quando as interfaces mudarem. O visual aprimorado foi arquivado;
suas configurações antigas são migradas para o otimizado sem apagar as demais opções.

## Base de desenvolvimento

- Projeto: `/mnt/ssd120/sacer/horizon-ntm`.
- Motor: `engine/horizon-ntm-next`, branch `ntm-current`.
- Upstream incorporado: `fc02103818c212920fd24aa34a2b2f91ba49e56f` (snapshot congelado de 03/11/2026).
- Motor de integração (pin): `f22df7b8127ae90f2cbd66dcfe5f58be70a9032b`.
- Pins do motor, mixins e Recipes: `native/ntm/compatibility.json`.
- K3 e energia solar/Stirling foram integrados nesta base; a antiga branch
  `ntm-next` continua preservada. Não usar seus pins antigos para novos desenvolvimentos.

O motor continua independente do NTM. Foram mantidos os modelos OBJ, animações,
multiblocos, tamanhos exatos de contêineres, filtros para menus/funis, dados de menu
com 32 bits e valores de jogador com 64 bits. A sincronização de block entities,
os componentes de itens, sons/partículas e páginas criativas aproveitam o upstream.
Essas extensões locais ainda exigem a revisão exata de integração: o número da API
oficial sozinho não garante compatibilidade.

## Compilar e atualizar

`tools/build-native.sh` encontra o motor relativamente ao projeto, consulta o
`main` oficial e exige que ele seja ancestral da revisão selecionada. Se o upstream
avançou, o script para indicando o novo SHA. Incorporar a atualização na branch
ativa, adaptar os pontos afetados, executar os testes e atualizar os dois pins
(`compatibility.json` e `horizon-mixin.toml`) antes de prosseguir.

O build usa `rsync` para conservar os arquivos intermediários idênticos e evitar
recompilar o motor inteiro quando só o bundle mudou. O alvo de compilação fica
fora da árvore intermediária sincronizada.

```sh
agent-run env QUEUE=cargo CARGO_BUILD_JOBS=2 \
  BUILD_DIR=/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/current-native \
  CARGO_TARGET_DIR=/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/migracao-upstream/target \
  bash tools/build-native.sh
```

O build nativo não regenera `dist/ntm.hmod`. Depois dele, empacotar o conteúdo da
mesma `main` com a ferramenta release recém-compilada, antes da aceitação em jogo:

```sh
agent-run env QUEUE=cargo CARGO_BUILD_JOBS=2 \
  HORIZON_MOD=/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/migracao-upstream/target/release/cargo-horizon-mod \
  bash tools/build-content.sh
```

Registrar os hashes do executável e desse pacote juntos; a presença de máquinas
antigas no pacote não comprova que ele contém os itens adicionados depois.

Os executáveis são escritos em `$CARGO_TARGET_DIR/release`. A aceitação exige
build release; `NATIVE_PROFILE=fast` serve apenas para iteração e não conta como validação. `SKIP_UPSTREAM_CHECK=1` só serve para repetir ou
reproduzir uma revisão já selecionada; sem essa opção, toda execução consulta o
Horizon oficial. O script não instala o jogo e não abre mundos do jogador.

O antigo `tools/test-game/run-visual.sh` é do protótipo WebAssembly; para esta
base, usar o executável produzido por `build-native.sh` com `dist/ntm.hmod`.
Nos testes, configurar `HORIZON_HOME`, `HORIZON_LAUNCHER_HOME`, `XDG_DATA_HOME` e
`XDG_CONFIG_HOME` em uma pasta descartável, com `HORIZON_NO_AUDIO=1`.
Executar o cliente tendo essa pasta descartável como diretório de trabalho:
caminhos relativos de mundos e cache também dependem dele.
Os roteiros com XTest precisam de uma janela visível dentro do Xvfb isolado;
nesses casos, omitir `--offscreen`, que oculta a janela e impede o foco do teclado.

Resultados da migração inicial estão no artifact `horizon-atualizacao`;
o fechamento seguinte está em `energia-solar-stirling-integracao`, com as provas
de K3, geração solar/Stirling e reabertura de mundo no snapshot acima.

## Auditoria das extensões — 02/11/2026, segunda atualização

O intervalo auditado foi `d928d1cd6..e8b55fbe5`. A primeira integração incorporou
`0e24521cd`; as consultas seguintes detectaram e incorporaram `693aa8482`,
`fbb17c980`, `ba832babb` e `e8b55fbe5`.
Motor de integração: `c8d86c4b5c2f3673d91445da5d5d906624bf6bed`. A revisão independente dos três conflitos não encontrou
problemas. Dano direto (`generic_kill`) e efeitos de radiação foram exercitados no servidor
integrado, além das GUIs e da bomba otimizada; a rodada desse pin passou (ver a terceira atualização abaixo).

| Extensão / caminho local | Resultado da comparação com o upstream | Decisão |
| --- | --- | --- |
| OBJ, ossos, animações, líquidos e culling (`mod_models`, `gfx/mod_models`, `block_entity_render`) | As mudanças oficiais tratam modelos vanilla, equipamento de mobs e itens soltos; não carregam o formato declarativo OBJ/animações usado pelo NTM. | Mantida. O desenho local foi combinado com a nova ordem oficial de sombras antes dos itens translúcidos. |
| Multiblocos (`horizon-world/mod_multiblocks`, `game/multiblocks`) | Não surgiu declaração de volumes, dummies, núcleo, alcance e quebra conjunta equivalente. | Mantida, incluindo a validação do clique nas células. |
| Tamanho exato de contêiner (`Kind::NativeContainer`, `lan/menu`) | Os menus oficiais seguem templates vanilla; não substituem a contagem arbitrária usada nas máquinas. | Mantida para preservar índices e slots sem arredondamento. |
| Filtros e limites por slot (`mod_slot_filters`, menus e funis) | Nenhum equivalente novo no intervalo de upstream; hooks de clique não garantem a mesma regra em previsão, servidor e automação. | Mantida uma regra compartilhada entre os três caminhos. |
| Dados de menu i32 (`native_hooks`, `lan/containers`) | O upstream ainda recebe palavras i16 no protocolo vanilla. | Mantidos os pares de palavras e a compatibilidade com servidores que só enviam i16. |
| Valores de jogador i64 (`native_players`, `player_values`) | Sem canal oficial equivalente para os medidores do NTM. | Mantido o payload genérico, sem reduzir a precisão do HUD. |
| Jogadores, inventários, dano e efeitos (`native_players`) | Os hooks novos observam eventos; não substituem os helpers em lote por UUID usados pelo NTM. | Mantidos, reaproveitando os comandos e efeitos oficiais internamente. |
| Mundo carregado, lotes estritos e sinal redstone (`native_world_access`) | Helpers oficiais de colunas e edição já existentes não preservam integralmente consulta carregada por dimensão, recusa atômica do lote e sinal recebido. | Mantidos esses contratos, sem duplicar geração de chunks. |
| Assets montados (`native_assets`) | O novo leitor `prepared_files` cuida de arquivos de recursos; não oferece a API restrita a `assets/`, listagem recursiva e aliases montados do bundle. | Mantida a interface local. O HUD adotou o leitor oficial para `.png.mcmeta` na resolução do merge. |
| Interação de bloco (`native_use_block`) | Não surgiu hook com posição clicada + núcleo multibloco + bloqueio da segunda mão. | Mantido para os controles e máquinas locais. |
| Entrada e relayout das telas de blocos (`NativeScreenEvent`, `native_screen_set_layout`) | Hooks oficiais genéricos já existiam; os commits novos não acrescentam um contrato equivalente com bloco, dados da máquina e relayout dinâmico. | Mantida a interface local e a entrega separada de teclas/caracteres. |
| Painéis de receita e busca (`creative_plus`, `native_panel`) | A correção oficial de teclas na busca criativa foi incorporada; não substitui o painel lateral de receitas nem o foco próprio do bundle Recipes. | Mantidos apenas os pontos locais necessários ao painel. |
| Rede de block entities, componentes de itens, som/partículas e páginas criativas | Já seguem os mecanismos oficiais integrados anteriormente. As novas alterações oficiais foram incorporadas. | Continuam oficiais; nenhuma segunda implementação adicionada. |

Nenhum substituto oficial novo cobriu integralmente uma das extensões locais neste intervalo.
Não foi removida uma extensão funcional para aparentar simplificação. A simulação in-frame foi
removida pelo próprio upstream; a resolução aceitou essa remoção e preservou o servidor integrado.

A regressão nuclear revelou um bug preexistente do upstream: o envio de `flash`
omitia a cor ARGB exigida pelo decoder. A integração acrescenta `Flash` ao caminho
oficial de cores e um teste separado de codificação/decodificação branco e verde,
preservando o teste upstream existente. É candidato a PR upstream; nada foi publicado.

Os roteiros antigos baseados em `/ntmbomba` dependem do protótipo WebAssembly.
No cliente nativo atual, a prova usa a bomba Little Boy montada com seus cinco
componentes, sinal de redstone e `saves/<mundo>/data/ntm/bombcfg` em uma cópia
descartável do mundo, com `mode: Otimizado`, `max_radius: 12` e fallout desligado.

## Terceira atualização — 02/11/2026, snapshot `2f35078b3`

Intervalo auditado: `e8b55fbe5..2f35078b3` (30 commits, 25 arquivos). A incorporação foi um
merge sem conflitos: `2ca67d71e`. O upstream já estava em `11440651b` quando o build rodou;
por isso o build usou `SKIP_UPSTREAM_CHECK=1` só para esse SHA congelado. Os avanços
posteriores ficam para a próxima rodada.

O delta traz: três hooks de jogador (`native_player_clone`, `native_player_load_from_file`,
`native_player_save_to_file`), corpos padrão em todos os métodos de `Host` (um
`unimplemented!("Host::nome")`, para dublês de teste), diretório `players/data` criado antes dos
hooks de arquivo, notificações de salvamento/entrada/saída no protocolo de gestão do servidor e
parada de itens, TNT e blocos em queda na borda do mundo.

| Extensão local | Equivalente novo no intervalo | Decisão |
| --- | --- | --- |
| OBJ/animações, multiblocos, tamanho exato de contêiner, filtros de slot, menu i32, valores i64, mundo carregado, assets montados, interação de bloco, telas de blocos, painéis de receita | Nenhum. O intervalo só mexe em jogador (hooks de clone/arquivo), `Host`, borda do mundo e protocolo de gestão. | Mantidas. |
| Jogadores, inventários, dano e efeitos (`native_players`) | Os hooks novos de clone/carregar/salvar observam eventos de ciclo de vida; o NTM não guarda estado próprio por arquivo de jogador e os helpers em lote por UUID continuam sem equivalente. | Mantida. Os hooks ficam disponíveis para persistência futura de dados por jogador. |
| Métodos de `Host` locais (API gerada) | Os corpos padrão que entraram valem também para os métodos locais, sem alterar o NTM. | Nada a trocar. |

Nenhuma extensão local foi substituída: não surgiu equivalente oficial novo.

Verificação do pin `2ca67d71e` (build release, sem testes em fast):

- Build release: passou em 7m20s.
- Aceitação em jogo descartável (Xvfb isolado, cliente release): catálogo K3 (340 verificações,
  94 itens), prensa elétrica, eletrolisador, radiação (efeitos e dano), Stirling e solar, e
  Little Boy otimizada (raio 12, cratera, `generic_kill` pelo dano nativo). O log não contém erro
  de decode do `flash`.
- Reabertura: o mesmo mundo foi encerrado e reaberto; o estado salvo das máquinas conferiu
  idêntico (entidades do roteiro Stirling/solar).
- Motor: 122 testes nativos, 25 de menus, 25 de renderização de mods, 57 de conteúdo
  (incluindo os ignorados), 4 de multiblocos, 2 de valores de jogador, 3 de partículas e
  2 do dublê de `Host`. Todos passaram.
- NTM: 295 testes Rust passaram (2 ignorados); Clippy `-D warnings` no workspace do NTM passou.
- Clippy `-D warnings` nas bibliotecas afetadas: passou na fonte original do motor. Na cópia
  transformada pelos mixins, três `collapsible_if` do código upstream aparecem porque a
  transformação remove os comentários entre os `if`; com esse único lint isentado, passou.
  Dois avisos novos do Rust 1.99 no código do próprio NTM foram corrigidos
  (`screens.rs`: `is_multiple_of`; `glue.rs`: referência desnecessária).
- Os testes nativos rodaram sobre a fonte original do motor, não sobre o código transformado.


## Quarta atualização — 02/11/2026, snapshot `2050c4e0f` — validado

Congelado no início em `2050c4e0f52b74ad5361f061e0a015e8bfc894b2`, 628 commits sobre
`2f35078b3`. Merge no motor: `18eab7ab41e767d9e21dc4079cb2f21dbe116c95`.
O build release usa `SKIP_UPSTREAM_CHECK=1` apenas para este snapshot.
Avanços posteriores serão tratados na próxima rodada.

| Extensão local | Decisão nesta rodada | Comparação com o snapshot oficial |
| --- | --- | --- |
| Replay pós-commit (`native/ntm/mixins/ntm/committed_updates.rs`, módulo do bundle em `horizon-world::level`) | Adicionada na rodada ZIRNOX de 03/11; sem mudar checkout/pin | `Sim/Behaviours::sim` são `pub(crate)` no upstream. Ponte mínima para concluir os updates de uma publicação atômica por meio do `Sim` existente. Reavaliar a cada atualização e remover quando houver entrada pública equivalente. |
| OBJ, ossos, animações, líquidos e culling (`mod_models`) | Mantida | As novas poses de mão, skins e quads de entidade não carregam os modelos declarativos OBJ do NTM. Capacidade do buffer combina modelos locais e contornos oficiais. |
| Multiblocos (`mod_multiblocks`, `game/multiblocks`) | Mantida | Sem contrato oficial para volumes, dummies, núcleo, alcance e quebra conjunta. |
| Contêiner de tamanho exato (`Kind::NativeContainer`, `lan/menu`) | Mantida | Hooks novos de inventário/bigorna não criam templates com número arbitrário de slots. |
| Filtros e limites por slot (`mod_slot_filters`) | Mantida | `menu_slot_rules` permanece um hook de menu; a regra local cobre também previsão e funis/automação. |
| Dados de menu i32 | Mantida | Protocolo vanilla continua i16; o par de palavras preserva os dados completos das máquinas. |
| Valores privados de jogador i64 (`player_values`) | Mantida | Novo sync de attachments aceita dados serializados, mas os envia também aos observadores do jogador, persiste com o codec e não oferece `sendToPlayer`. Não preserva o canal efêmero exclusivo do dono. |
| Jogadores, inventário, dano e efeitos (`native_players`) | Mantida | Hooks novos observam/cancelam eventos; não substituem consultas e ações em lote por UUID. O escoamento após o ServerTickEnd permanece. |
| Visita completa de nível (`Game::visit_level`) | Mantida | `levels.rs` oficial não mudou no intervalo. Entrada normal ainda zera idle e limpa partículas/pickups; a visita restaura tudo inclusive em unwind, sem abrir dimensão. |
| Mundo carregado, lote estrito e redstone (`native_world_access`) | Mantida | Helpers oficiais `edit_blocks`/`set_block` não oferecem o mesmo contrato por dimensão aberta, validação prévia do lote e leitura de comparador/lid_openers do nível completo. |
| Assets montados (`native_assets`) | Mantida | `mod_image` oficial envia RGBA ao atlas e `instance_dir` retorna a pasta; nenhum substitui leitura restrita a assets montados com aliases/listagem. |
| Interação de bloco (`native_use_block`) | Mantida | `right_click_block` não entrega o núcleo do multibloco nem resolve o consumo entre as duas mãos; controles continuam no hook local. |
| Entrada e relayout de telas de blocos (`NativeScreenEvent`) | Mantida | Novos hooks de abrir/inicializar/fechar observam telas por frame; não fornecem o evento local com bloco, dados da máquina e alteração dinâmica dos slots. |
| Painel de receitas e foco na busca (`creative_plus`, `native_panel`) | Mantida; tooltip adotou a função oficial | Painel/foco local ainda necessário. O merge usa `draw_stack_tooltip` oficial, com a exclusão local do segundo tooltip na tela criativa. |
| Flash (`lan_particles`) | Mantida, candidata a PR | O arquivo oficial não mudou. O encoder ainda omite ARGB de Flash sem o conserto local; branco e verde continuam cobertos por roundtrip. |
| Rede de block entities, componentes de itens, som/partículas e páginas criativas | Seguem oficiais | Mantidos os mecanismos já adotados; novas mensagens oficiais de attachment e melhorias upstream incorporadas. |


Verificações concluídas na fonte original do motor (sem transformação pelos mixins):

| Grupo | Resultado |
| --- | --- |
| Nativos (`native_`) | 125 passaram. |
| Menus | 26 passaram. |
| Renderização/mods | 26 passaram. |
| Conteúdo, incluindo testes ignorados | 61 passaram. |
| Multiblocos | 4 passaram. |
| Protocolo i64 | 2 passaram. |
| Partículas | 3 passaram; o roundtrip de flash também pertence ao filtro nativo. |
| `mod_api` e integração tipada | 3 + 2 passaram. |
| NTM | 300 unitários + 26 de integração passaram; 2 doctests ignorados. |
| Clippy estrito | `-D warnings` passou nas 11 bibliotecas selecionadas do motor e no workspace NTM. |
| Cópia transformada | Somente os 3 `collapsible_if` conhecidos em `game.rs`; todos os demais lints passaram ao isentar só esse lint. A fonte original passa sem isenção. |

Build release: Cargo concluiu em **10min26s** (26 injeções, 5 módulos). O conteúdo
foi reempacotado com o `cargo-horizon-mod` desse release: `build-native.sh` sozinho
não o gera. A primeira tentativa usou um pacote anterior sem `hand_drill_desh`;
essa tentativa ficou arquivada e a aceitação em jogo foi reiniciada integralmente
com o pacote da `main` atual.

Upstream mais novo visto na consulta final: `1411020cbb161f608db9171bc1c70aad1ed002a3`.
Não foi incorporado: está reservado para a próxima rodada, conforme a política de congelamento.


A persistência da pilha exige ar positivo antes da captura. O ensaio longo de
operação pode esgotar o barril e o vent; no primeiro ensaio ambos estavam em zero
antes e depois da reabertura, embora a asserção exija `mb > 0`. O driver desta
rodada repõe ar pelo contêiner e pelo botão normais do barril, confirma o vent
positivo e só então captura o estado. `native_pile.py` e suas asserções permanecem
inalterados; não há injeção de estado/NBT nem redução da verificação.


Aceitação final com o pacote reempacotado e o mesmo cliente release:

| Cenário | Resultado |
| --- | --- |
| Catálogo K3 | 340 verificações, 94 itens. |
| Prensa elétrica / eletrolisador | 7 + 4 verificações. |
| Radiação | 2 verificações: efeitos e dano nativo observados. |
| Solar/Stirling | 15 verificações; 21 estados idênticos após salvar, encerrar e reabrir. |
| Little Boy otimizada / flash | 10 verificações: raio 12, cratera, dano `generic_kill`; logs sem erro de decode. |
| `native_dims.py` | 6 verificações na montagem + 4 após reabrir, incluindo dois fornos novos recebendo energia das máquinas retomadas. |
| `native_dims.py idle` / `idle-clean` | 2 + 1 verificações; a dimensão sem jogador fecha e salva com e sem leitor. A fumaça segue apenas como cenário visual. |
| `native_pile.py` | 23 de operação + 25 de reabertura + 31 de destruição, incluindo falha no Overworld com jogador no Nether. Precondição de ar positivo verificada pelo driver (mais 1). |

Os roteiros do repositório foram executados sem alterações; os dez scripts do
usuário continuam byte a byte iguais e fora do commit. Não houve instalação,
push nem uso de mundos reais. Todos os clientes e Xvfb da aceitação foram encerrados.
O OpenGL do Xvfb continua sem TAA; essa rodada valida os fluxos funcionais/UI,
não TAA nesse backend. Os testes são direcionados à integração, não a suíte
completa de paridade vanilla.

Evidências desta rodada: `.agent-tmp/upstream-2050/` (em relação à raiz do projeto),
com `snapshot.json`, `validated-build.json`, `rust-results.json`, `game-summary.json`
e logs. O conjunto aceito tem:

- Motor: `18eab7ab41e767d9e21dc4079cb2f21dbe116c95`.
- SHA-256 do cliente: `e01fe940ff37a2e83f880ef2da74515c35020e60924c640a1a0b2cf7fa022893`.
- SHA-256 de `dist/ntm.hmod`: `d8cd6303742230d743e04220ac1da6af8a3f3bfacbf80a237dafe7b93870e130`.

Pendências: incorporar o upstream posterior ao snapshot em outra rodada; propor
separadamente o conserto genérico do flash ao upstream; os três lints da cópia
transformada continuam sendo uma diferença do transformador. Nenhum ajuste de API
foi necessário no código Rust do NTM: esta adaptação altera apenas os pins e a documentação.

## Quinta atualização — 03/11/2026, snapshot `fc0210381` — validado

Snapshot congelado no início: `fc02103818c212920fd24aa34a2b2f91ba49e56f`, **1.242 commits** após `2050c4e0f`.
Motor de integração: `ec479adb3a66977b339672a7ff8747a7e951068e`; base NTM: `cbcc3006d7661300133f9bbd0cb9a2f0610df1aa`.
`SKIP_UPSTREAM_CHECK=1` foi usado somente para este snapshot; avanços posteriores
ficam para outra rodada. Não houve push nem criação de subagentes.

O merge resolveu dois arquivos: o fechamento criativo conserva o painel local e
a limpeza oficial do foco de controle; os hooks de filtro e contêiner de item
coexistem. Os três hooks locais passaram aos contratos e despachos do gerador
tipado oficial. O atributo recursion_limit do snapshot quebra o mixins fixado:
foi removido e os JSONs aninhados de mobs foram separados sem mudar seus campos.
As novas fixtures upstream recebem filtros/modelos vazios, preservando suas asserções.

As três isenções antigas de collapsible_if da fonte transformada foram eliminadas
com condições equivalentes no motor. Clippy ainda encontrou uma quarta condição
no glue Stirling, ajustada também. A rodada preliminar foi arquivada, e **toda a
aceitação foi reiniciada com o release final**, um único cliente e pacote.

| Extensão | Decisão em fc0210381 | Evidência e limite do mecanismo oficial |
| --- | --- | --- |
| OBJ, ossos, animações, líquidos, culling | Mantida | O novo `horizon-assets/obj.rs`/loader OBJ oficial resolve geometria estática/material para modelos baked. Não lê normais `vn` (o leitor local mantém normais por vértice) nem a composição por ossos, keyframes e líquidos sincronizados. A nova propriedade oficial render_shape seleciona a forma de desenho, mas não cobre por si só o noOcclusion/transparência e a descoberta automática dos arquivos horizon_model/liquid. Substituí-lo aqui degradaria iluminação/animação. |
| Multiblocos | Mantida | Sem declaração oficial de volumes, dummies, núcleo, alcance e quebra conjunta equivalente. |
| Contêiner de tamanho exato | Mantida | Os menus oficiais declarados (`horizon/menu`, já presentes em 2050c4e0f) permitem 1–256 slots. Exigem menu registrado e native_screen; o caminho local também atende contêiner de contagem exata sem registro/tela, e preserva os índices na previsão. Trocar apenas Kind perderia esse contrato; não há substituto novo neste intervalo. |
| Filtros/limites compartilhados por slot | Mantida; despacho trocado pelo oficial | `menu_slot_rules` cobre menus. `Rules::may_pickup` agora protege cliques de hóspedes, mas não é política nativa compartilhada de extração por item/funil. `native_slot_filters` adotou contrato collect e despacho gerado oficial. |
| Menu i32 e valores privados i64 | Mantidas | Os pacotes vanilla de menu ainda são i16. Attachments/EntityRef NBT não equivalem ao canal efêmero preciso exclusivo do dono nem aos pares de palavras existentes. |
| Jogadores/inventário/dano/efeitos em lote por UUID | Mantida | EntityRef ganha velocidade, bounds, dados/tags; não substitui o conjunto de ações/consultas em lote por UUID do NTM. |
| `visit_level` e `native_world_access` | Mantidas | `LevelRef` agora lê estados/propriedades/altura/tempo de níveis abertos, inclusive estacionados. Não oferece visita mutável completa, restauração em unwind, lote validado por inteiro, sinal de comparador/lid_openers e manutenção do idle/partículas. `native_edit_blocks` continua limitado ao nível em tick e sem updates de vizinhos. |
| Assets montados | Mantida | `mod_image`, instance_dir e o novo loader OBJ não substituem leitura restrita a assets montados, aliases e listagem recursiva. |
| Interação no núcleo multibloco | Mantida; despacho trocado pelo oficial | `right_click_block` não resolve núcleo/célula clicada e consumo entre as mãos. `native_use_block` adotou cancel/claimed no gerador oficial. |
| Eventos e relayout de tela de BE | Mantida; despacho trocado pelo oficial | Eventos genéricos de tela não carregam bloco + dados i32 + relayout dos slots. `native_screen_event` adotou cancel/claimed e tipo exportado pelo mod_api oficial. |
| Painel de receitas/foco criativo | Mantida | Novas categorias de receitas declaram tipos/ícones; o próprio contrato diz que ainda não adiciona tabs/displays. Não substitui o painel lateral/foco Recipes. Merge preserva também pad_clear_focus oficial. |
| Flash (`lan_particles.rs`) | Mantida | Mudança oficial neste arquivo apenas troca partículas de blocos por track_explosion. ARGB de Flash continua ausente no encoder oficial. Roundtrip local branco/verde preservado. |
| `[[module]] committed-world-updates` | Mantida | `Sim` e `Behaviours::sim` continuam pub(crate); `set_block` retorna false se o novo estado já foi publicado. Nenhuma entrada pública pós-publicação realiza as callbacks flag 3 depois de reter o conteúdo do BE atomicamente. O módulo delega ao Sim existente. |
| `world_saved` | Oficial mantido, sem duplicação | API 306 existe no snapshot, hook chamado após save_level, save_parked_levels e metadata; wait indica flush. O bundle apenas injeta confirmação/limpeza dos recibos. Não é transação atômica mundo+arquivos do mod. |
| Rede de BEs, componentes de itens, som/partículas, páginas criativas | Oficiais mantidos | Seguem os mecanismos oficiais já integrados. |

Pendências auditadas:

- `stillValid` por identidade do BE: #7973/#7880 acrescenta `BlockEntity::fits` e troca BEs incompatíveis com o bloco. `Slots::valid` ainda só verifica presença na coordenada e `still_valid` acrescenta alcance: não fixa identidade/geração da instância aberta. Correção útil incorporada, contrato completo continua pendente.
- `can_take` condicional por item: #7845 propaga `Rules::may_pickup(slot, stack)` nos cliques de hóspedes e inclui regressões; a API 334 já existente `native_inventory_move_item(source, destination, item, cancel)` permite cancelar a transferência por item no funil (e 352 observa o começo da tentativa). É um recurso oficial útil para a automação futura, porém seu caminho curto exige trait mods registrados (`mods().is_empty()` pula a chamada em um bundle só de mixins). O funil em `game/machines.rs::can_take` continua baseado em outputs para contêiner de mod; ainda não há política única por slot/item/dados que cubra a previsão de menu, servidor e funil.
- Fechar menus de BE: `native_close_menu` ainda recusa menus que não sejam `Slots::Native`; hooks `player_container_close` observam o fechamento. Não há helper oficial equivalente para fechar arbitrariamente o menu de BE do NTM por identidade.
- Save atômico: autosave de arquivos dos jogadores sai da thread, falhas de save são reportadas/serializadas e world_saved(wait) permanece. Nenhuma transação entre regiões/metadata e os arquivos do mod; retém-se o protocolo NTM de efeitos/recibos e suas limitações de queda abrupta.
- Entidades de mod: `EntityRef` agora fornece velocidade/on_ground/bounds/olho/distância, NBT, tags e nome, com handles por epoch; somadas a summon/displays/impactos existentes, ajudam a usar entidades vanilla. O conteúdo já permite entity_type derivado de snowball (item arremessado, dano fixo, rede e save por chunk); displays/falling blocks também já existem. São opções oficiais para projéteis simples, mas não equivalem à física, dano/térmica, recuperação e desenho dos detritos ZIRNOX. O intervalo não ampliou ENTITY_TEMPLATES além de snowball; não migrar os detritos cosméticos sem preservar esses contratos.

Compatibilidade adicional: o snapshot introduz recursion_limit no crate root, rejeitado pelo mixins ddd9700 fixado (E_UNSUPPORTED_MACRO). Removido localmente conforme o mesmo mecanismo da PR oficial ainda aberta #8193, dividindo também os JSONs aninhados do controle de mobs para compilar no limite padrão; não se avançou o snapshot. O novo gerador exige mod_api para cada hook: os três hooks locais passam por contratos/despachos oficiais, sem alterar os pontos de injeção do NTM.

| Verificação | Resultado |
| --- | --- |
| NTM | 369 unitários + 26 de integração passaram; 2 doctests continuam ignorados. |
| Conteúdo Python | 218 passaram, recursos 26.3 explícitos. |
| Motor, fonte original | 139 nativos; 36 menus; 34 render/mods; 66 conteúdo (incluindo os ignorados); 4 multiblocos; 2 i64; 3 partículas; 4 mod_api; 3 hooks tipados; 15 gerador. Todos passaram. Há sobreposição entre filtros; não somar como testes distintos. |
| Clippy e formatação | Fonte original, NTM (all-targets) e cliente release transformado passaram com -D warnings e sem isenções. rustfmt passou. |
| Updates flag 3 | Teste na cópia transformada passou: chest/redstone_block/stone comparados ao setBlock normal, conteúdo retido, luz/heightmaps, observer e lâmpada. |
| Build | Release, 31 injeções e 6 módulos, incluindo Recipes. Conteúdo reempacotado com a ferramenta desse release. |
| Mineração | 18 verificações, incluindo escavadeira, poço e torre de fracking. |
| Bombas | 49 verificações, incluindo Little Boy/redstone, detonador e pavio. Nenhum erro de decode do flash. |
| ZIRNOX | 110 verificações: montagem, inatividade, conversão, reabertura, falha e solos. Traço físico: uma explosão aplicada, centro [115,104,6], potência 12 e 72 detritos criados. |
| Pile | 83 verificações, incluindo reabertura/destruição e precondição de ar positivo pela UI normal. |
| Dimensões | 10 + 2 idle + 1 idle-clean; três mundos novos separados. |
| Catálogo K3 | 340 verificações e 94 itens. |
| Solar/Stirling | 15 verificações e 21 estados idênticos após encerrar/reabrir. |
| Complementares em jogo | Prensa elétrica 7; eletrolisador 4; efeitos/dano de radiação 2. |
| Recibos | Em cada um dos sete mundos dos roteiros nativos, machine_effects, applied_machine_effects, removals e prepared_retentions vazios após o encerramento normal. |


Cliente: `.agent-tmp/zirnox/native-target/release/horizon-client`, SHA-256
`912c112ffbf1d6743d878b0f66e7dcf1ca3e76ad572c764f7328f4a9abc940ae`. Conteúdo: `dist/ntm.hmod`, SHA-256 `eb96b19246c029df3be847d25c1ae068f88b177940a9b48cc9b00635dbcab631`,
9.103.885 bytes (igual ao pacote da referência ZIRNOX atual).

Evidências relativas à raiz do projeto: `.agent-tmp/upstream-fc021/`, especialmente
`snapshot.json`, `validated-build.json`, `rust-results.json`, `game-summary.json`
e logs. O agregado confere as mesmas fases, identidades e quantidades de checks de
`.agent-tmp/zirnox/validation-generation.json`; os horários/ticks não são fixados. Quatro famílias de checks de UI incluem o número dinâmico de passos/polls no nome: o comparador normaliza somente esse sufixo e preserva a multiplicidade de cada asserção.
A revisão estática pelo próprio executor está no artifact `revisao-upstream-fc021`.

Os dez arquivos locais do usuário foram preservados byte a byte e ficaram fora do
commit. Foram usados somente mundos descartáveis e Xvfb isolado. Os caches de
jogo desta rodada foram removidos após extrair as provas; reutilizaram-se os
caches Cargo existentes, sem criar targets adicionais. Os avisos de TAA no
OpenGL/Xvfb continuam esperados; essa aceitação funcional/UI não valida TAA nem
substitui a suíte completa de paridade vanilla.

Pendências: incorporar futuros snapshots em outra rodada; enviar separadamente a
correção genérica do flash ao upstream; acompanhar a correção oficial da
compatibilidade recursion_limit/mixins (#8193); os contratos de stillValid por
identidade, can_take unificado, fechamento de menu de BE, transação de save e
entidades NTM continuam nos limites descritos acima. Nenhuma isenção de Clippy
permaneceu nesta rodada.


## Onda 0 — política de inventário e portas (marco 1)

Motor `d5eb69a33a5c2d137d620865dd1e39e1e31aa38c`, derivado do pin
`ec479adb3` (`47173f4c4` e quatro correções da revisão); upstream congelado `fc0210381`. [Contrato e exemplo da
API](native-slot-policy.md).

| Extensão local | Razão e composição com o oficial |
| --- | --- |
| `ModSlotFilter::policy`, `SlotAction`, `SlotOrigin` | O hook `slot_filters` existente recebe um callback puro com inventário atual, índice local, stack completo, ação e origem Manual/Automation. Previsão, menus autoritativos, shift-click, bundle, drag, troca, descarte e funil consultam a mesma regra. `Rules::may_pickup`, `inventory_move_item`, filtros estáticos, limites e faces continuam ativos. |
| `Multiblock::inventory_ports` e `inventory_core` | Resolve apenas dummies declarados como portas até o contêiner do núcleo vivo. `Containers::view` compartilha o comportamento com funis, carrinhos e `native_insert_item`. O núcleo conserva o inventário próprio; não-portas e núcleos sem BE não expõem contêiner. |

O ZIRNOX habilita saída automática nos slots 0..23 somente quando
`zirnox::can_extract` permite; a exclusão provisória da extração foi removida.
As cinco portas seguem `ReactorZirnox.getAllPorts`. O funil vanilla entra pela
porta superior e extrai pelo núcleo inferior. Sob cada porta lateral há
invólucro, que impede a instalação física de um funil extrator também no Java.
A trilha RTG/radgen ligará suas próprias funções depois da integração deste
marco, sem cherry-picks entre branches.

### Correções da revisão independente (P3)

| # | Correção | Commit do motor |
| --- | --- | --- |
| 1 | `bundle_click` na LAN exige `may_place` na retirada parcial para a trouxa, como `menu/bundle.rs` e `try_remove`. | `3e479f8b0` |
| 2 | `swap` (teclas 1–9 e `F`) na LAN divide a pilha até `slot_max`, como `Menu::swap`: o resto fica na mão/hotbar e a pilha antiga vai ao inventário ou ao chão. | `88327ab6c` |
| 3 | Troca direta (cursor ocupado, teclas 1–9, `F`) avalia `may_place` com o slot já vazio depois de `may_pickup`, no cliente e na LAN. Mesclar o mesmo item continua vendo o slot ocupado. | `f27edd9de` |
| 5 | Os slots 0..23 do ZIRNOX são `Slot` vanilla no Java: a inserção manual é irrestrita (uma unidade). Barras ficam exigidas na automação (`slot_allows`) e no shift-click, por `ModSlotFilter::quick_move`. | `7607685b8` (motor) e `1e62763f` (NTM) |

O item 5 exigiu um campo novo no motor: sem ele, remover o `accept` estático
deixaria o shift-click mover qualquer item para as barras, o que o
`transferStackInSlot` Java não faz. Os slots 24 e 25 (recipientes de CO₂ e de
água) são `Slot` vanilla no Java e receberam o mesmo tratamento depois: mão
livre, só recipientes do fluido certo pelo shift-click e nada pela automação
(`isItemValidForSlot` é falso para `i >= 24`). Os slots 26 e 27 seguem
`SlotTakeOnly`.

Validação das correções (após as quatro, mesmo binário em todos os roteiros):
release `horizon-client` `539b1e144b59c4ffb173e76e946e32f800bcfeda0c903444933704b2cf11f87e`,
pacote `dist/ntm.hmod` `c2eda3dc2bc98b7f9d710b3bd3c3e3db39a84509515e7fae23c439b9a6894321`
(9.103.922 bytes, idêntico ao anterior: nada mudou no conteúdo), pin `7607685b8`.

- **Motor:** 139 `native_`, 43 `menu::tests::` (inclui 21 de `lan::menu`), 5 `mod_multiblocks`, 35 `mod_` (render), 2+4 `conditional_policy`; Clippy `-D warnings` sem isenções nas bibliotecas, `cargo fmt --check` e Clippy da cópia transformada (stage).
  Testes novos: `guest_bundle_partial_take_needs_the_slot_to_accept_placement`, `limited_slot_splits_oversized_offhand_and_hotbar_swaps`, `conditional_policy_sees_the_slot_empty_when_a_stack_replaces_it` e a versão `guest_`, `quick_move_filter_narrows_shift_click_but_not_hand_placement` e a versão `guest_`.
- **NTM:** 372 unitários + 26 de integração (2 ignorados), Clippy `-D warnings` do workspace sem isenções. Teste novo: `zirnox_manual_insertion_is_unrestricted_but_automation_and_shift_click_take_rods`.
- **Em jogo (`--scratch-root proofs`):** `zirnox` (`zirnox-pvmk0r95`, 127 checks, 0 falhas, inclui `automation` e as novas `manual_non_fuel_*`: terra substitui uma barra viva no slot 300, a barra volta inteira, a terra volta ao inventário e a vida da barra não muda), `inventory-proxy` (`inventory-proxy-29egtrei`, 12), `bombs` (`bombs-43v377jt`, 49), `pile` (`pile-p90_s6q8`, 83), `dims` (`dims-s28w2hfs`, 10), `dims-idle` (`dims-idle-jsq4d1ed`, 2), `dims-idle-clean` (`dims-idle-clean-1fsb1cwp`, 1), `mining` (`mining-yzmarqd8`, 18) e `general.py` (`general-x18sezpx`, catálogo K3 340 checks/94 itens + 49 de prensa, eletrolisador, radiação, solar/Stirling e reabertura).
  Uma execução anterior do `zirnox` (`zirnox-2muh4e3g`) falhou no roteiro novo, que procurava um slot de barra vazio quando os 24 estão cheios; o roteiro foi corrigido para substituir a barra do slot 300 e a execução acima é a do roteiro final.
- Limite: não há roteiro de dois clientes, então a LAN foi coberta só pelos testes de `lan::menu`; nenhum roteiro em jogo exerce o hóspede LAN.

Registro anterior ao pacote de correções (binário `515b953d702e…`, pin `47173f4c4`):
`inventory-proxy-iz5l5y6f` (12 checks), `zirnox-2xubdn0h` (123 checks, com a primeira
prova de `automation`: 11 `zfb_mox_depleted` + 1 trítio extraídos, 0 no binário
anterior `912c112ffbf1…`), `mining-50ih28gu`, `bombs-jjh4t8wg`, `pile-9ibhz_3e`,
`dims-34f1wlwd`, `dims-idle-k1pkjqvu`, `dims-idle-clean-dnrbdizz` e `general-dnj_x0kf`.


## Onda 0 — itens 2 a 4 (menu por identidade, itens estacionados, formação da pilha)

Motor `d5eb69a33a5c2d137d620865dd1e39e1e31aa38c` (sobre `7607685b8`); NTM rebaseado em `fb93c7cf` (RTG/radgen).

| Extensão local | Razão e composição com o oficial |
| --- | --- |
| `BlockEntities::identity` (`horizon-client-render`) | Número dado a cada objeto de block entity: mantido na reescrita no lugar e no `take`/`put`, perdido na remoção, na troca de tipo e no descarregamento; nunca reutilizado. É o `level.getBlockEntity(pos) == this` do `stillValid`. `BlockEntity::fits` (#7973/#7880) continua responsável por trocar BEs incompatíveis com o bloco; a identidade cobre a troca por outro objeto do mesmo tipo. |
| `lan::menu::Menu::bound`, `Hub::bind_menu`, `Menu::valid` e `Menu::still_valid` | O menu aberto guarda as identidades dos BEs da abertura (`lan_actions.rs`, depois de `open_container`). Cliques, botões (`mod_menu_button`), shift-click nativo e `sync_containers` exigem a mesma identidade; um menu cujo BE foi removido e recolocado fecha e seus pacotes em trânsito são ignorados. Veículos e menus avulsos não são fixados. |

Prova no motor: `identity_follows_the_object_not_the_position` (unidade) e `a_window_pinned_to_a_replaced_block_entity_takes_no_clicks`
(`tests/lan_bot.rs`, hub real: a janela antiga recebe um clique depois de quebrar e recolocar o mesmo baú e não muda nada; sem o
`bind_menu` o teste falha em `the old window is stale`; uma janela aberta depois funciona). Em jogo (`native_dims.py`, `identity_close`):
com `tick freeze`, o menu de um forno elétrico fecha quando ele é quebrado e outro do mesmo id é colocado, e o menu da máquina nova abre.

Item 3 (itens em nível estacionado): ver `host-dimensoes.md`. Item 4 (formação da pilha): `Server::pile_form` monta as escritas
(tijolo → bloco, primeiro JSON e dados do BE novo em `CellWrite::initial`, desgaste da broca em `wear_held`) e `Host::commit_formation`
as publica em `publish_atomic`, sob o acesso exclusivo ao `Game`. Preflight: células ainda tijolos simples, broca ainda na mão.
Recusa deixa tijolos, modelo e ferramenta como estavam (`formation_publishes_blocks_first_state_and_wear_together_or_not_at_all`,
`formation_with_an_unloaded_cell_changes_nothing`). Em jogo (`native_pile.py`, `formation_wear`): uma broca comum forma uma pilha em
sobrevivência e perde exatamente 1 de durabilidade.

Validação (mesmo binário `a79e93f2f051b30ad48a998b922d3a62f219718f7087f6382e2403b6895a9287`, pacote `b18682da6df57763ecf5955ca9bdc276c4cb314661aebb3ee7af0f3ee9d8f06e`):

- Motor: 139 `native_`, 43 `menu::tests::`, 18 `lan_bot`, 5 `mod_multiblocks`, 35 `mod_`, 7 `block_entities`, 2+4 `conditional_policy`; Clippy `-D warnings` e `fmt` limpos.
- NTM: 389 unitários + 26 de integração (2 ignorados); Clippy do workspace e do stage transformado sem isenções.
- Em jogo (`proofs/`): `dims-g3eloyq0` 14, `zirnox-cg0pza3j` 133, `inventory-proxy-vfu15rte` 12, `radioisotopes-sr5ezwd4` 67 (setup, decay, reopen e automation), `bombs-t6g29o6c` 49, `pile-h_lkne1n` 85, `dims-idle-oidsl1u7` 2, `dims-idle-clean-1dhjpnjg` 1, `mining-haaayl08` 18, `solar-cy9s9yup` 36, `combustion-ywdes7bf` 15, `catalog-4dth6s9s` 251. Nenhuma falha.
- Limites: o menu fixado só é exercitado por pacotes reais no `lan_bot`; em jogo o teste fecha o menu do jogador local. O item 3 não tem leitura de comparador (ver `host-dimensoes.md`).


### Correções da revisão do marco 2 (P3)

Motor `d5eb69a33a5c2d137d620865dd1e39e1e31aa38c`; NTM `wave0-integrity` com `662c5bf9` e `0364228a`.

- **Comparadores (P3 #1).** A bateria, o tanque de fluido e o barril de aço (os portados cujo Java tem `hasComparatorInputOverride`)
  respondem à API 64 (`has_analog_output`, `analog_output`) com as fórmulas do Java, e o glue chama `native_update_output_neighbors`
  quando o sinal muda num commit de máquina, também em nível estacionado (`visit_level`). Ver `host-dimensoes.md`. As demais máquinas
  com a propriedade no Java (BAT9000, tanque grande, soquete, caixas, Geiger, controle MK1) ainda não foram portadas.
- **Shift-click do ZIRNOX (P3 #2).** `ModSlotFilter::quick_inserts` deixa um slot `output` que nomeia a lista receber o item pelo
  shift-click; o CO₂ vai ao slot 24 e transborda para o 26, a água ao 25 e depois ao 27, como o `mergeItemStack` do 1.7.10.
  Testes: `quick_move_reaches_an_output_slot_that_names_the_item` (e a versão `guest_`), `zirnox_manual_insertion_is_unrestricted_but_automation_and_shift_click_take_rods`;
  em jogo, `shift_click_overflows_into_the_take_only_slot`.
  O Java também enche as barras de trás para a frente (`mergeItemStack(0, 24, true)`); o motor enche da frente. Não alterado (sem efeito
  de jogabilidade: todas as células aceitam qualquer barra).

Validação (binário `dc0eea70c23dd42d85f4497a865867d20114bd90089c1ce91dfbc16c67d2f172`, pacote `b18682da6df57763ecf5955ca9bdc276c4cb314661aebb3ee7af0f3ee9d8f06e`):

- Motor: 139 `native_`, 45 `menu::tests::`, 18 `lan_bot`, 7 `block_entities`, 35 `mod_`, 5 multiblocos, política condicional; Clippy `-D warnings` e `fmt` limpos.
- NTM: 416 testes (389 unitários + 27 de integração), 2 ignorados; Clippy do workspace e do stage transformado sem isenções.
- Em jogo, sem falhas: dims 14, zirnox 134, inventory-proxy 12, radioisotopes 67, bombs 49, pile 85, dims-idle 2, dims-idle-clean 1,
  mining 18, solar 36, combustion 15, catalog 251, comparator 7.


### Fechamentos adicionais da revisão (ordem das barras e rede de baterias)

Motor `d5eb69a33a5c2d137d620865dd1e39e1e31aa38c`.

- **Ordem do shift-click.** `ModSlotFilter::quick_reverse` marca slots que um shift-click visita de trás para a frente dentro de uma
  corrida de slots consecutivos (`quick_visit_order`), como `mergeItemStack(0, 24, true)`; os slots fora da corrida seguem a ordem
  normal. O ZIRNOX marca os 24 slots de barras. Previsão e host LAN usam a mesma ordem (testes
  `quick_move_fills_a_reverse_run_back_to_front` e a versão `guest_`); em jogo, `shift_click_fills_the_rod_slots_back_to_front`.
- **Baterias não trocavam energia pelo cabo (causa e correção).** A rede do port (`energy::distribute`) fazia três passos:
  geradores→consumidores, geradores→armazenamento, armazenamento→consumidores. Nunca armazenamento→armazenamento, então duas
  baterias no modo Buffer não trocavam nada. **Isso não é fiel ao Java.** `PowerNetMK2.update` junta todos os provedores
  (`min(power, getProviderSpeed())`) e todos os receptores (`min(max-power, getReceiverSpeed())`) num só conjunto, e
  `TileEntityBatteryBase.updateEntity` põe uma bateria em Buffer nos dois conjuntos (`addProvider` e `addReceiver`);
  `TileEntityMachineBattery.getProviderSpeed` é `max/600` e `getReceiverSpeed` é `max/200`. Uma bateria cheia alimenta uma vazia.
  Acrescentei o quarto passo (armazenamento→armazenamento, só com duas ou mais, para não girar sobre si mesma), com as mesmas
  taxas. Como no Java, duas baterias com a mesma oferta e o mesmo espaço pesam igual e param de trocar. Teste:
  `batteries_exchange_energy_with_each_other`; em jogo, a segunda bateria do `native_comparator.py` sobe a partir de 0 enquanto a primeira desce.
- **Observação, não alterada:** no Java a bateria nova começa com `redLow = 0` (entrada apenas) e `redHigh = 2` (saída com redstone);
  o port começa em Buffer/Saída. Mudar isso quebraria os fluxos existentes (a bateria criativa alimentando máquinas sem redstone);
  fica registrado para uma decisão de produto.

Validação desta rodada (binário `ec4feef1ac29177e5dd6f126cdf1229e2d77fa857b737e41b2478bac6f25966c`, pacote `b18682da…f06e`):

- Motor: 139 `native_`, 47 `menu::tests::`, 23 de `lan::menu`, 18 `lan_bot`, 7 `block_entities`, 35 `mod_`; Clippy `-D warnings` e `fmt` limpos.
- NTM: 417 testes (2 ignorados), Clippy do workspace e do stage sem isenções.
- Em jogo, sem falhas: dims 14, zirnox 136, inventory-proxy 12, radioisotopes 67, bombs 49, pile 85, dims-idle 2, dims-idle-clean 1,
  mining 18, solar 36, combustion 15, catalog 251, comparator 9 (inclui a transferência bateria→bateria).
- O `pile` falhou uma vez em `meltdown_dismantles_pile`; a causa está abaixo (era o roteiro) e não é mais intermitente.

### Fechamentos finais: bateria como no Java e o meltdown do `pile`

NTM `wave0-integrity` sobre o motor `d5eb69a33a5c2d137d620865dd1e39e1e31aa38c` (sem mudança no motor nesta rodada).

- **Modo padrão da bateria.** `TileEntityMachineBattery` nasce com `redLow = 0` (só entrada) e `redHigh = 2` (saída com redstone); `Battery::new`
  agora também. O `native.rs` teste `the_battery_fills_from_its_slot_and_turns_its_modes` confere o estado inicial `[.., 0, 2, ..]` e os cliques
  da tela (`redLow`: entrada → buffer → saída → nada). A bateria criativa é um item (`IBatteryItem`), sem modo próprio: não mudou.
  Os roteiros que usam uma bateria como fonte agora fazem o que um jogador faz: um bloco de redstone sob a bateria
  (`Game.battery_output`) em `native_dims`, `native_comparator`, `native_stations`, `native_redes` e `native_recipes`.
  As que só armazenam (combustion, solar/Stirling, RTG/radgen) seguem como estavam.
- **`pile` / `meltdown_dismantles_pile` — causa.** Não é defeito da varredura de órfãos. Evidências:
  1. O teste de unidade `native_meltdown_uses_the_engine_blast_and_never_drops_rods_twice` agora exige que **todo** `pile_block` reverta no tick
     seguinte ao do núcleo (antes conferia uma célula).
  2. A sonda `pile-meltdown` (novo cenário; servidor congelado, `tick step` de 5 em 5 até o núcleo reverter, depois de 1 em 1): em 5 rodadas o
     varrimento reverteu as 101 células de pé exatamente **um tick** depois do núcleo, e as demais vezes já estava feito dentro do mesmo passo de 5.
  3. O tick do estouro (explosão de potência 15 mais as atualizações de bloco) prende o servidor integrado por vários segundos: leituras de
     células a cada 40 ms viram 42 a 75 `pile_block` de pé durante os primeiros 5 s e zero depois, e `time query` chegou a passar de 10 s sem resposta.
     O roteiro lia o mundo 3 s (relógio de parede) depois de ver o núcleo reverter, antes de o tick do varrimento rodar.
  Correção do roteiro: `destroy` espera o varrimento (até 90 s) em vez de dormir 3 s; `Game.ctl` repete um comando que o controlador deixou
  sem resposta por causa de um tick pesado; a sonda `pile-meltdown` fica como regressão por ticks.
  Observação de desempenho, fora do escopo: o estouro do meltdown prende o servidor por mais de 10 s neste host (render por software).
- **Execuções do `pile`.** Cinco seguidas no binário final, `pile-b1` a `pile-b5`, todas com 85 checks e sem falha, mais cinco antes do ajuste
  do `ctl` (quatro passaram; a quinta caiu num `execute` que o servidor ocupado não respondeu em 10 s, o defeito do `ctl` corrigido acima).
- `native_recipes.py` e `native_stations.py` não fazem parte da bateria padrão; `stations` roda (5 checks); `recipes` (montadora) falha
  na abertura do menu com ou sem esta mudança (verificado por A/B), por posicionamento do próprio roteiro antigo.

Validação (binário `e57ab62508c8c7c9bbdf5283fc3794882b462b0d9654c23e8aefc9a8fdf5f3d4`, pacote `b18682da…f06e`):

- NTM: 417 testes (2 ignorados), Clippy do workspace e do stage sem isenções. Motor sem mudança desde `d5eb69a33` (testes da rodada anterior).
- Em jogo, sem falhas: dims 14, zirnox 136, inventory-proxy 12, radioisotopes 67, bombs 49, pile 85 (×5), pile-meltdown 32, dims-idle 2,
  dims-idle-clean 1, mining 18, solar 36, combustion 15, catalog 251, comparator 9, stations 5.

## Onda 0 — item 5 (save atômico)

Branch NTM `wave0-vanilla` sobre `main` `85d06d9c`; motor `wave0-vanilla` `a31497f38` sobre `ntm-current` `f22df7b81`:
instrumentação de teste (os failpoints do save, o alvo `live_chunk`, `HORIZON_WRITER_HOLD`) e duas extensões locais
(`world_saving` e a descarga com o chunk no lugar; seção seguinte). **Estado: entregue para revisão independente, sem
merge.** A sétima revisão e a segunda rodada da revisão fresca reprovaram o R6 (R7-1 a R7-4, N1, N2); todos foram fechados
na raiz. Backups: NTM e motor `wave0-save-r4c` (`891d02f0`, `1d48f96aa`).

**Critério.** Cada execução é comparada com o vanilla no mesmo roteiro e no mesmo corte, **item por item**: (L) nada some
que o vanilla guarde; (D) nenhum item fica com mais cópias que no vanilla. Nenhuma violação é dispensada por categoria.

**Princípio: toda obrigação mora no registro do chunk que a originou, nunca no KV** (a semântica de um BE vanilla). E nada
é dado a um registro depois que um save o tomou.

| Parte | Como fica |
| --- | --- |
| Armazenamento (KV, `data/ntm/state`) | Em memória; gravado inteiro no fim de cada save (`.tmp` e `rename`; `fsync` com espera), depois de `native_world_saved(wait)` com o `wait` real do save (R7-1). Guarda só o que é do mundo e não decide o conteúdo de nenhum chunk: bombas, radiação, ticks, zirnox, gerações. **Nenhuma remoção, lote de efeitos, recibo ou sobrevivente** de uma versão nova. |
| Obrigações no registro do chunk (`server::obligations`) | Uma remoção que espera seu grupo e um lote de efeitos em voo moram no anexo `ntm:ledger` da sua casa: o chunk da máquina removida, ou o do primeiro dono do lote (o núcleo numa pilha). O servidor dá a cada registro o que ele deve no mesmo tick em que muda (`store_obligations`), e só joga o que uma casa carregada deve (`home_loaded`); quando ela carrega, o registro diz o que ainda deve (`native_obligations_loaded`). |
| Mudança entre ticks (N1, R7-2) | Um bloco mudado depois do último tick (um comando, um tick congelado) é tomado como o fim de um tick o toma (`native_settle`: eventos, só os passos dos grupos cuja máquina ou núcleo saiu, efeitos, obrigações): antes de o save tomar qualquer coisa (`world_saving`) e na descarga, com o chunk ainda no lugar. O conteúdo de uma máquina removida sai na hora, como o de um baú. A regravação tardia de registros depois das regiões foi removida. |
| Quebra de um núcleo | Solta as hastes na hora, do chunk do núcleo, como o `breakBlock` do Java; membros fora de alcance voltam a tijolo quando carregam (varredura de órfãos). Nenhuma remoção espera segurando itens. |
| Nomes dos lotes (R6-1) | Derivados de um nonce sorteado por sessão e de uma contagem: um KV que volta com uma morte não repete o nome que um registro ainda guarda como pago. |
| Emissão | Chunk do drop carregado: drop vanilla no mesmo passo. Não carregado: devido no registro do dono (o slot de saída), pago quando o chunk entra. |
| Multibloco rasgado | O núcleo com uma célula estranha se desmonta; membros sem núcleo voltam a tijolo (`a_pile_saved_formed_on_one_side_of_a_border_reopens_whole_or_as_bricks`). |
| Saves antigos: o ledger | Migração única e idempotente do ledger rastreado (R5-1); um registro ainda não resolvido atravessa podas e saves inteiro (F2) e sua contagem é ajustada pelo parser SNBT (F3). |
| Saves antigos: remoções e lotes do KV (R7-1, N2) | Ficam no KV **como foram lidos** (`WorldState::legacy`), não como a fila está, e nenhum é jogado antes de o registro da casa ser lido (`ServerHost::home_migrated`). Um registro que já o tomou (`migrated: X`, guardado com o registro) diz o que resta dele, e a cópia do KV não acrescenta nada. Senão o servidor o assume e o registro passa a dizer `migrated: X`. O KV o descarta só quando um save **com espera** gravou o registro com o marcador antes dele (capturado em `world_saving`), ou quando o registro foi lido do disco com o marcador. Um save sem espera não aposenta nada. O marcador sai quando o KV aberto com o mundo já não o nomeia. Um lote antigo sem nome recebe um derivado do seu conteúdo, igual em toda leitura. |
| Recibos e sobreviventes da sessão | O que um save levou com os chunks é confirmado quando os registros estão no disco: na hora, num save com espera; num sem espera, no primeiro tick em que o escritor do motor diz que tudo o que recebeu está no disco (`writes_finished`). Achado do meltdown no R7b: confirmados só por save com espera, cresciam sem fim numa sessão de autosaves, e o passo de cada grupo os copiava (pior tick de 61 ms na sexta rodada). |
| Bombas (F4, R7-3) | Fiel ao HBM 1.7.10, quirks incluídos: a cratera de uma nuke (`EntityNukeExplosionMK5`) para com o mundo, e o balefire (`EntityBalefire`) continua. O dano de cada uma dura exatamente a sua cratera (o pulso guarda o id da explosão), curta ou longa, sem prazo fixo; ver `docs/bombas.md`. |

**Verificador** (`crates/ntm-core/tests/save_model.rs`). Chunk A da máquina (BE, devidos, nomes pagos, marcador), funil,
um terceiro chunk, os arquivos de entidades, o jogador, o KV gravado no instante do save com as regiões ainda na fila, e o
escritor do motor. A máquina atira duas vezes (em sessões diferentes) e pode ser quebrada: por um jogador (espalha na hora,
como o dropper vanilla) ou por um comando entre dois ticks (`Command`), que o mod só toma no fim do próximo tick, antes de
um save tomar os chunks, ou numa descarga com o chunk no lugar. Um tiro devido pago deixa seu nome no registro de A. O
mundo também abre de um KV antigo com uma remoção ou um lote. O análogo vanilla da remoção é um baú inteiro no registro de
A, quebrado no instante em que o mundo lê esse registro; o do lote é a saída de um dropper. Cada sequência roda no mod e no
vanilla, passo a passo; (L) e (D) por átomo a cada estado, sem exceções.

| Execução | Resultado |
| --- | --- |
| N=9 | 188.661.834 estados visitados, 12.399.394 distintos, 8.361.560.872 sequências; zero violações. |
| **N=10** | **1.437.623.114 visitados, 92.279.811 distintos, 116.107.959.595 sequências; zero.** 10.783 s. (O arquivo do log foi sobrescrito por uma segunda execução, interrompida; o resultado foi restaurado do log da shell monitorada, com nota.) |
| KV antigo (remoção e lote) | Profundidade 9: remoção 137.589.262 visitados, 8.415.470 distintos, 9.804.079.312 sequências; lote 279.953.704 visitados, 17.630.320 distintos, 10.284.021.767 sequências; zero. |
| Passeios aleatórios | 6 milhões de até 40 operações (240 M operações) e, com outra semente, 6 milhões de até 60 (360 M); zero. Os passeios acharam uma divergência do próprio modelo (o nome do derrame de um comando), minimizada por delta-debugging a 10 operações e corrigida no modelo. |
| Morte fora de um save | Os dois mundos voltam ao último save, átomo por átomo. |
| Mutante `InStorage` (devido no KV) | Pego em 129.834 estados. |
| Mutante `RemovalInStorage` (R6-2, F1) | Pego em 310.728 estados. |
| Mutante `CounterIds` (R6-1) | Pego em 43.155.679 estados. |
| Mutante `PublishAfterCapture` (N1: publica depois da captura) | Pego em 357 estados: `[Fire(A), Command, Save(true), Kill]`. |
| Mutante `UnloadWithoutHome` (R7-2: unload sem casa) | Pego em 33.775 estados: `[Fire(A), Command, Unload(A), Unload(funil), Save(true), Kill]`. |
| Mutante `RetireEarly` (R7-1: migração aposenta antes) | Pego em 137 (remoção) e 138 (lote) estados: `[…, Save(false), Kill]`. |
| Mutante `NoMarker` (N2: sem o marcador) | Pego em 6.606 estados: `[…, Pickup, Save(false), Write, Kill]`: o estoque sai duas vezes. |

O verificador achou duas divergências reais no caminho. A primeira (R6): com a remoção que esperava o grupo, o mod
guardava as hastes no registro de A, enquanto o vanilla as espalhava nas entidades. Daí a quebra imediata. A segunda (R7),
no próprio desenho da migração: a primeira versão comparava a remoção antiga com hastes já espalhadas nas entidades de A, e
a descarga (entidades, depois registro) dava duas cópias. O análogo certo é o baú inteiro quebrado na leitura, que tem o
mesmo resíduo de ordem da descarga vanilla. Os sete mutantes ficam como testes permanentes. Os testes de unidade
(`pile_server`: `a_legacy_*`, `a_marker_read_from_the_disk_*`, `a_break_not_yet_taken_when_its_chunk_unloads_*`,
`a_break_with_no_tick_before_the_save_*`) cobrem cada corte com o código real, e cada um falha com a mutação
correspondente aplicada ao código (aposentar sem espera, ignorar o marcador, não acomodar antes do save).

**Provas em jogo.** **129/129** pontos em 24 cenários no binário final, duas reaberturas, cada uma comparada **por
item** (também a segunda, R7-4), cada cenário jogado também no controle vanilla. Todo cenário tem `kill` (morte fora de
um save) e **`kv_committed`**: um save sem espera (a pausa) com o escritor de regiões parado (`HORIZON_WRITER_HOLD`) e o
processo morto logo depois do KV (`NTM_FAILPOINT=kv_committed`), o corte "KV novo, regiões antigas". O oráculo compara o
**multiconjunto** de itens por id (hastes por tipo, devidos e remoções dos anexos incluídos) no disco do corte, depois de
cada reabertura e contra o controle, e exige `kv_items = 0` no disco dos dois mundos. Um corte por índice de registro é
comparado com o controle que gravou os mesmos chunks, em todas as condições (F5).

| Grupo | Pontos | NTM = controle |
| --- | --- | --- |
| `original` (todos os cortes, `kv_committed`, `live_chunk:2,0`/`3,0`, varredura) | 18 | 12, ou 2 quando o núcleo chegou sem as entidades (os mesmos chunks gravados no controle) |
| `mining` (todos os cortes, `kv_committed`, `live_chunk:1,0`) | 16 | 7, ou 0 em `chunks_written`/`live_chunk:1,0`, como o baú |
| **`frozen-break`** (N1: núcleo quebrado por comando com ticks congelados, save sem tick no meio) | 7 | 12 = baú em todos os cortes; 2 = 2 em `live_chunk:2,0`/`chunks_written` (as hastes nas entidades ainda não gravadas, como os itens do baú) |
| **`frozen-unload`** (R7-2: a mesma quebra, e o chunk do núcleo descarrega antes de um tick) | 3 | 12 = baú |
| **`legacy`** (R7-1 e N2: KV no formato R5 com um lote devido ou já entregue, cortado por `kv_committed`, por `players_saved`, regiões novas com KV antigo, ou inteiro) | 6 | 1 niter em cada uma das duas aberturas, por item, e depois de pago; o KV do corte o guarda (só um save com espera o aposenta) |
| `coupled-a/b/ea/eb` | 24 | iguais ao funil em cada corte |
| `coupled-break` (R6-2) | 6 | 6; em `chunks_written`/`entities_written` o mod guarda as hastes devidas no registro que o vanilla perde nas entidades não gravadas (6 contra 1), sem cópia por item |
| `break`/`replace`/`break64` | 21 | iguais ao baú (16 ou 64; 0 nos cortes que gravaram só o chunk do baú) |
| `save-quit`, `crossing`, `crossing-kill` | 8 | 12; 24 e 20 = controle (entidade que atravessou para um chunk descarregado) |
| `origin-*`, `merge-stale`, `pickup-merge`, `origin-away` | 20 | iguais ao controle; em `kv_committed`, 32 e 176: o arquivo do jogador gravado pela pausa à frente das regiões (resíduo vanilla) |

Rodadas em paralelo (até 10 jogos), cada falha reexecutada sozinha antes de contar: 2 provas (`origin-pickup-away`,
`origin-away`) falharam por tempo no paralelo e passaram sozinhas.

**Resíduos, todos iguais ao vanilla na mesma execução:** o save rasgado entre chunks; o arquivo do jogador à frente das
regiões; a transferência rasgada na borda; o devido que volta com o estoque depois de uma morte; a entidade que
atravessou para um chunk descarregado; a descarga rasgada entre o arquivo de entidades e o registro (a do baú quebrado,
também para a remoção de um KV antigo assumida na leitura). Obrigações de uma dimensão estacionada chegam quando ela volta
a ser a ativa e o chunk recarrega.

**Desempenho do meltdown** (seis rodadas, `HORIZON_SPIKES`, pior tick por rodada em ms):

| Execução | Rodadas |
| --- | --- |
| B1 (base) | até 2.273 |
| R4c | até 359 (espera do disco no KV por tick) |
| R5f | 13/48/71/15/65/68 · 14/43/64/18/46/70 · 13/38/58/13/45/111 |
| R6f | 8/19/28/7/19/33 · 5/18/24/6/14/25 · 6/14/22/6/17/26 |
| R7b | 7/17/31/43/53/61 · 7/18/25/38/48/59 · 8/18/28/42/47/61 (cresce a cada rodada: os recibos, corrigido no R7c) |
| R6b, mesma janela | 5/14/25/6/15/24 · 8/20/29/7/22/32 |
| **R7c**, intercalado com o R6b | **9/22/31/8/23/32 · 8/20/29/7/20/28** |

As medições de R6b e R7c da última linha foram intercaladas (R6b, R7c, R6b, R7c), numa janela em que a coordenação parou
as outras frentes: nenhuma compilação nem outro jogo, carga média de 1 minuto entre 1,8 e 6,5 no início de cada uma (o
próprio jogo largo, nas CPUs 0–4 e 6–10). Os números absolutos não se comparam com os do R6f, medido noutra configuração
da máquina; o que decide é a tendência: o R7c cai a cada autosave (quarta rodada) como o R6b, e fica na faixa dele.
Medições anteriores sob a carga de outras frentes (até 13,8, com compilações) deram picos nos dois binários, como um
`keep_world` de 501 ms no R6b, e não entram na conclusão.

**Validação final.** Motor `a31497f38`; binário `horizon-client.R7c` SHA256
`98a85002ec50c70a0ac860ea3ef60618e5ed2d8c6fa924cc83c0a5d517b24ed0`, construído de `9c4061a5`. NTM: 522 testes em debug
(os do verificador só em release), verificador em release (11 testes); motor: 10 testes de `save::`/`entities::` e o da
ordem do save, 3 de `world::` (o da descarga) e 4 de ganchos; Clippy `-D warnings` do workspace NTM, do
`horizon-client-lib`, do render (lib e testes) e do cliente transformado. Regressão no mesmo binário: 28 execuções, 30
mundos, **1.943 checks, nenhuma falha** (13 execuções falharam por tempo no paralelo e passaram sozinhas). Código de
produção NTM (`crates/ntm-core/src` e `native`, com os testes de unidade): contra o R6b (`a2003c95`) 16 arquivos,
+800/−205; contra `wave0-save-r4c` 19 arquivos, +2010/−677. Motor contra `f22df7b81`: 7 arquivos, +278/−14.

Evidência local: `wave0/crash-reports/` (`R7cf-proof-summary.json`, `R7c-regression-summary.json`, `P-R7c-*`, `P-AB-*`,
`P-R6b-ab`, `P-R7b-*`), `wave0/opus/model-R7b-{9,10,walks2}.log`. Mundos aprovados são apagados após guardar logs e números.

## Onda 0 — item 5: extensões locais do motor

Motor `a31497f38` sobre `f22df7b81` (`0f21c07f0` era só instrumentação). Ambas são candidatas a PR upstream. O estudo do
upstream mostrou que `host-bridge` e `chunk-tickets` mexem perto delas: conferir esses dois na próxima atualização antes de
reaplicá-las.

| Extensão local | Razão e composição com o oficial |
| --- | --- |
| Gancho `world_saving(wait)` (`native_world_saving`, em `native-hooks.json`), na cabeça de `save_game_with`, antes de `save_level`, dos níveis estacionados, dos metadados e dos jogadores | O upstream só tem `world_saved` (API 306), depois de tudo tomado. Todo save (autosave, pausa, `save` do controle, `/save-all`, fechamento, dedicado) passa por `save_game_with`, e nada do motor roda entre um comando do controle e esse save; um tick congelado nem chama o `server_tick` do NTM. Sem o gancho, a quebra de uma máquina com conteúdo oculto ficava fora do save que a seguia (N1). Custo nulo sem mod: um despacho vazio por save. Testes: `a_save_is_heard_before_it_takes_anything` (ordem em `save_game_with`) e `world_saving_reaches_the_mods_with_its_wait` (despacho). O efeito ponta a ponta é a prova `frozen-break`. |
| Descarga com o chunk no lugar: `native_chunks_changed` devolve a `world.chunks` as colunas que o `streaming.update` já tirou (`World::put_back`) durante o despacho de `chunk_unload` e as tira de novo, com o que os ganchos escreveram, para o `unloaded` que é gravado (`World::take_back`) | O upstream retira as colunas antes de avisar os mods, e o que um mod gravasse no chunk que sai não chegava ao registro gravado (R7-2). O `ChunkEvent.Unload` do NeoForge vem com o chunk ainda acessível, antes do save. Custo: duas operações de mapa por coluna descarregada. Teste `a_column_put_back_for_its_unload_leaves_with_what_was_written_into_it` (o anexo e o bloco escritos durante o gancho estão no chunk gravado). Prova em jogo `frozen-unload`. |

## Hooks de item — extensões locais

| Extensão local | Equivalente no upstream congelado `fc0210381` | Decisão |
| --- | --- | --- |
| `save_live_later` e `HORIZON_FAILPOINT=live_chunk:x,z` | O writer já distingue chunks vivos de protos; os ordinais antigos contam ambos. | Diagnóstico aprovado pelo coordenador. O alvo vai primeiro em sua região e o processo cai após seu header, somente no lote vivo de `save_level`. Desligado por padrão; ordem normal e gameplay preservados. O harness exige no `.mca` exatamente um dos chunks novo e o outro antigo. |
| `native_player_transactions`: snapshot por UUID/época, CAS de inventário completo, armadura, offhand e attachment com codec; fome, cura, efeitos e cooldown após validação | Hooks oficiais de uso/equipamento/dano e attachments já existem. Não oferecem commit conjunto nem recusa sem consumir parte do inventário. | Extensão local; reutiliza persistência de attachments, efeitos, cooldowns e eventos oficiais. Nenhum tipo do NTM entra no motor. |
| `remote_open_screen`: id validado do bloco de origem no BE temporário de menu customizado | O upstream deixa o id vazio e perde os filtros condicionais no caminho de previsão. | Preservar o id apenas quando o bloco clicado declara o mesmo menu; políticas compartilhadas seguem idênticas no servidor e no cliente. |
| `Hub::sync_containers` e `Menu::sent_carried`: o cursor que o mod publica depois de um clique em menu nativo | O upstream reenvia o menu e o cursor antigos quando o clique vai ao mod (`resend_menu`) e só compara slots na sincronização do tick. Um cursor alterado pelo commit do mod nunca chegava ao cliente. | Extensão local, restrita a `Slots::Native`: o hub guarda o último cursor enviado e manda `SetCursorItem` quando difere. Teste `a_native_menus_cursor_published_after_the_resend_is_synced`. |
| `Game::native_use_claimed` e `Hub::send_inventory` depois de um `UseItem` | O cliente do hóspede prevê o uso vanilla (equipar a armadura) antes de o hook nativo do servidor tomar o uso, e nada corrigia a previsão. | Extensão local: quando um mod toma o uso, o servidor reenvia o inventário completo do hóspede. |
| `NativeDropMode` e `Game::native_drop_item`: mão, lançado (`func_146097_a(.., false, false)`, com a estatística) e `EntityItem` no mundo | O upstream tem `Player.drop` (mão) e `throw_from_eyes(.., false)`, sem a estatística nem o `EntityItem` com o salto do construtor, nem escolha de modo por chamador. | Extensão local; os commits de jogador publicam uma lista de `NativeDrop`. Fórmula do salto em `world_item_velocity`, com teste; origem verificada no decompilado do 1.7.10 (`docs/native-item-hooks.md`). |
| `Menu::native_resend` e `Hub::flush_native_resends` | O hub reenviava o menu e o cursor *antes* de o mod responder ao clique (estado anterior ao clique), e o estado novo só saía no tick seguinte. | Extensão local: o clique só marca; depois de `run_native_menu_events` o hub envia o menu e o cursor já com a resposta, no mesmo tick. Teste de hub. |

Duração/animação/start/finish e tick de jogador usam exclusivamente hooks oficiais.
A aceitação desta extensão e os limites estão em `docs/native-item-hooks.md`.


## Correções da montadora e do meltdown da pile

Branch `fix-assembler-meltdown`, rebaseada sobre `main` @ `4bb53ca7`; motor `0615f5db0` (`ntm-current`, hooks de item), upstream congelado `fc0210381`.

### A montadora não abria o menu: era o roteiro

O commit `6e3f3f1e` passou a abrir o menu pela célula sul do núcleo (z=10). `Game.open` fica 3,5 blocos ao sul dela (z=13,5), mas `Game.ground` só põe o piso até z+4=12: o jogador caía de y=100 para y=76, fora de alcance, e o menu nunca abria (o roteiro leu `status.player` por 1,5 s depois do `tp` para provar isso). O item na mão e a latência foram medidos e descartados (abre em 0,14 s com e sem o item). Nada no runtime ou no menu estava errado.
`Game.ground` ganhou `depth` (padrão 4; só a fase `montadora` usa 6) e `recipes` roda `montadora` e `bigorna`, de volta à regressão.

### O estouro do meltdown prendia o servidor por mais de 10 s: diário gravado n vezes

A explosão (`native_explode`, potência 15) custa pouco. O tick ficava em 8 a 12 s de relógio com 0,4 a 1,5 s de CPU e a lista de spans vazia: a thread do servidor ficava em estado D na syscall `rename` (`folio_wait_bit`, `blk_mq_get_tag`, `ext4_wait_block_bitmap`).
`Server::flush_machine_effects` regravava o diário inteiro (`world`, 1,9 a 3,9 MB; `write` num `.tmp` e `rename`) **antes de cada efeito**. A varredura de órfãos do tick seguinte enfileira um lote com 124 efeitos `Blocks(1,1,0)`, um por membro, e gravava 125 vezes: custo quadrático (n gravações × O(n) bytes). O ambiente de teste agrava, porque `agent-run` usa `ionice -c3` em ext4/BFQ com pressão de E/S alta, mas o custo quadrático existe em qualquer disco.

**Invariante (o que permite omitir uma gravação).** Pode-se pular a gravação anterior a um efeito apenas se: (1) uma gravação da mesma chamada já guarda a fila inteira; (2) o efeito refrescado é igual ao guardado; (3) nada que uma reexecução repetiria (itens, explosão, remoção com `retained`) saiu desde essa gravação. `Blocks` simples reexecutam sem dano porque o guarda compara a célula como está. A gravação depois do último efeito continua. Quem mover o diário para outro lugar (registro do chunk, save atômico) deve manter o custo por tick independente do número de efeitos.

**Provas.**

- `the_meltdown_sweep_journals_once_and_ends_as_a_write_per_effect`: o mesmo meltdown no modo estrito (gravação por efeito, mantido só em teste por `Ready::strict_journal`) e no novo. Blocos, drops, explosão e o `world` persistido saem idênticos; a varredura vai de 125 para ≤ 3 gravações e os bytes caem mais de 20 vezes.
- `an_effect_that_must_not_repeat_is_journaled_before_the_next_one_goes_out`: depois de um efeito de itens, a gravação seguinte já tem o item como feito.

**Números** (3 meltdowns por execução do roteiro `pile-meltdown`, mesmo host, mesma carga de fundo):

| Medida | Antes (`73d7b842…`) | Depois |
| --- | --- | --- |
| Pior tick do servidor | 8,0 s; 10,5 s; 11,7 s (CPU 0,4 a 1,5 s) | 0,97 s (CPU 0,23 s); sem tick acima de 200 ms no binário final |
| "o servidor não respondeu em 10 s" | sim | nunca |
| Gravações do `world` na execução inteira | 378 (~116 por meltdown) | 75 |
| Servidor bloqueado em `rename` | 22,7 s | 1,9 s |
| Gravações no tick da varredura (teste de unidade) | 125 (134,8 MB) | ≤ 3 |

Limite: ficam ~12 gravações por meltdown vindas de outros caminhos do diário, não investigados; a CPU restante (~0,2 s no tick do estouro) não foi comparada com uma explosão vanilla de mesma potência.

### Validação

Binário `horizon-client` release `d554e57887fbd1dcf04ce9fcc460d5b362b9c87dfc0d4ab043be26a6b524e043`, pacote `dist/ntm.hmod` `3fbcad5823f75e54fd3e944780eaeec2e59f016bb7691a6e76c74919bfb3ca53` (reempacotado com o `cargo-horizon-mod` desse release). Build release em 11m39s (`SKIP_UPSTREAM_CHECK=1`).

- NTM: 423 testes passaram (2 ignorados). Clippy `-D warnings` do workspace, todos os alvos, sem isenções. Não foi executado o Clippy do stage transformado: a glue (`native/ntm/mixins`) não mudou.
- Em jogo, mesmo binário, sem falhas: items 58, recipes 5 (o JSON final é o da bigorna; a montadora também passou), pile 85, pile-meltdown 32, zirnox 136, bombs 49, dims 14, dims-idle 2, dims-idle-clean 1, mining 18, radioisotopes 67, inventory-proxy 12, solar 36, combustion 15, catalog 251, comparator 9, stations 5.
- Uma rodada anterior, sobre o binário do pin `d5eb69a33`, teve falhas por disco cheio (`/mnt/ssd120` a 100%, causado por outros diretórios de trabalho) e uma abertura de menu do `radioisotopes` sob carga extrema; repetidas com espaço livre, passaram. Esta rodada, no binário final, passou de primeira.

## Onda 1 — consumíveis, kits e medicina (K2)

Sem mudança no motor (pin `0615f5db0`). O que o NTM ganhou e onde está documentado:

| Extensão local | Razão |
|---|---|
| `ntm_core::consumables` (`consumables.json` e `kits.json`) e o glue `item_hooks.rs` | O mesmo commit atômico do jogador (`native_commit_player`) aplica a tabela de uso de cada família; ver [Hooks nativos de item](native-item-hooks.md) e [o catálogo K2](content-catalogo-k2.md). |
| `ntm_core::blast::vanilla::allocate_vnt`, `cross_hit` | `ExplosionVNT` do `chocolate_milk` pelo motor de explosões do NTM (resolução 32, força até 64, dano e empurrão do `EntityProcessorCross`); mobs e itens soltos esperam a API de entidades. |
| `Server::rad_buf`, `note_contamination` e `zone_rate` | A `radBuf` por jogador vem da mesma leitura que move a dose (sem caminho paralelo), na janela de um segundo, com os `contaminate` pontuais somados à taxa contínua: Geiger, dosímetro. |
| Mob effects de conteúdo `ntm:radx`, `radaway`, `stability`, `death`, `potion_sickness`, `taint` | Estados passivos que o Java lê com `isPotionActive`; o motor não tem tick para efeitos de conteúdo. |



## Sons próprios do HBM — 04/11/2026

Base NTM rebaseada sobre `main` `9f11740a` (inclui consumíveis K2). Motor local
`f22df7b8127ae90f2cbd66dcfe5f58be70a9032b`, derivado de `0615f5db0`;
upstream congelado `fc0210381`, sem mudança do protocolo. Pins em
`compatibility.json` e `horizon-mixin.toml` atualizados juntos.

| Extensão local | Suporte oficial reaproveitado | Razão da extensão |
| --- | --- | --- |
| `Ticking::External` no `SoundEngine` | `Sounds`, handles posicionais e atualização de volume/pitch/posição já existem | Manter instância atualizada pelo dono cliente, derivada do snapshot de BE, até stop/release explícito; não adicionar protocolo de loops. |
| `BlockProperties::door_sounds` | Loader de conteúdo e templates de `SetType` | Escolher eventos de interação open/close para uma porta declarada por um mod; validação de namespace e restrição a portas. |
| Categoria por caminho de qualquer namespace em `settings::sound_source` | Canais/categorias oficiais do áudio | O teste anterior só removia o prefixo minecraft e tratava `ntm:block.*` como master. |
| `HORIZON_TRACE_SOUNDS` opt-in, com filtro de namespace | `SoundRegistry` e decoder Ogg oficiais | Provar no log resolução para arquivo real do mod e decodificação, sem mudar o RNG normal. |
| `HORIZON_AUDIO_TEST_MIXER=1` opt-in | Mixer/decoder e control socket oficiais | Exercitar canais e PCM reais sem hardware; expor cursores, ganho/pitch e histórico de ciclo de vida só no modo diagnóstico. |

Carregamento de `assets/ntm/sounds.json`, Ogg, pesos, variantes, referências,
volume/pitch, stream, atenuação e subtitles já é oficial; não foi criado outro
registry. `native_play_sound` aceita nomes de mod por `Holder.direct`.
Os loops usam o update tag oficial dos BEs; a onda nuclear inicia por mod messages
oficiais. Detalhes, inventário Java gerado, licenças e limites em
[sons-hbm.md](sons-hbm.md). Quatro eventos excluídos por aviso Sound Dogs; CC0 de
qubodup creditado no próprio pacote. `entity.vomit` permanece silencioso como no
original, sem alias para player.vomit.

Validação após revisão: 452 unidades + 26 integrações = 478 aprovados, 2 doctests
ignorados; Clippy NTM workspace/all-targets com `-D warnings`, sem isenções.
Python: 258 contratos de conteúdo + 8 do conversor/guarda de áudio. Motor: 19
testes de backend + 8 SoundEngine; Clippy do motor/bundle lib/bins/tests passou.
O limite preexistente do exemplo no all-targets adicional continua documentado.

R1/R2/R4/R5: splash exclusivo do Well, ID de upgrade `machine_well`, spin industrial
anterior ao débito e guarda de licença por todos os campos corrigidos. A guarda
recusa negações/conflitos e só excepciona o aviso conhecido do próprio GoldWave.

`HORIZON_AUDIO_TEST_MIXER=1` mantém o mixer/decoder reais ativos sem abrir hardware,
com canais, PCM, cursor, ganho, pitch e histórico stop/destroy no status. Esse é um
modo diagnóstico opt-in; `HORIZON_NO_AUDIO=1` permanece nos testes. A prova pelo
dono NTM passou 68 verificações. O handle nuclear agora para e libera ao fechar ou
trocar o mundo. A porta produz 2 traces de resolução, mas 1 canal da janela e 1
transição por interação; o teste distingue essas cardinalidades. LAN com dois
clientes continua na onda 7.

Release de aceitação: código NTM `4771ab2b`, motor `f22df7b81`;
binário SHA-256 `aac79a3bec5e21a40eba1139b075abbc3bdb109dbea6a2659b1132201e51bc76`;
pacote SHA-256 `9ab18eacc7e9e1573d18439d3d675191eb9aaaa43a949300bd7dfe36a3cee0aa`.
A nova regressão integral concluiu 19/19 relatórios e 1.022 verificações em
`proofs-review-v4`: 68 da prova ativa, 13 de resolução/decoder e 941 gerais.
Os 17 cenários CLI retornaram 0; dims inclui idle e idle-clean. Todos usam o mesmo
binário; inventory-proxy usa sua fixture oficial, e os outros 18 usam o pacote NTM. Detalhes em [sons-hbm.md](sons-hbm.md).

## RBMK T01 — contrato transacional sobre o save atômico (09/11/2026)

Branch `rbmk-t01` sobre `main` `91e309f5` (save atômico R7c), com `front-rbmk-core` `589ca13a` mergeado em `94f2af00`
(decisão D-a do coordenador); motor `a31497f38`, sem mudança de motor. Ticket T01 do artifact `rbmk-plano`.
Três partes, todas sobre o contrato do R7c (toda obrigação no registro do chunk que a originou; nada dado a um registro
depois que um save o tomou):

| Parte | Como fica |
| --- | --- |
| Transferência jogador↔BE (B) | `ServerHost::player_slots` lê as pilhas do jogador que age (cada uma com o SNBT inteiro) e `commit_player_transfer` publica num passo só: a glue confere todo o lado das máquinas (slots, geração, células), publica o lado do jogador por `native_commit_player` (CAS do inventário inteiro, cursor, mão secundária e attachment; recusa não muda nada) e então as máquinas, cuja publicação não falha depois das conferências; tudo no mesmo callback com `&mut Game` exclusivo. Uma pilha editada mantém os demais componentes. Hosts sem suporte recusam: não há caminho que debite um lado depois do outro. Primeiro usuário: o carregador da pile, cuja barra saía da mão depois do commit (`PileUse::ConsumeRod`); agora sai na mesma transferência. O segundo é a barra RBMK na mão (`RBMKRod.onBlockActivated`: copia uma, debita só fora do criativo, `upgradePlug`). |
| Recibos de entidades (C) | `WorldEffect::Entities` (tipo, posição exata, compound SNBT) segue o caminho dos itens: com o chunk de destino carregado, `apply_entities` invoca as entidades por `native_summon`; senão ficam devidas (`ChunkLedger::owe_entities`) no registro de um dono carregado e saem quando o destino carrega. O id do efeito (`serial`,`done`) impede o segundo spawn num replay. Host sem entidades recusa: o lote espera e o dono congela, nada se perde. O tipo de detrito do RBMK é do T20 (D9). |
| Grupo acoplado com RNG/fila (A) | Cada camada RBMK (dimensão, Y da base) é um grupo de `steps()`: suas colunas (BEs) e o estado da camada no `WorldState` (KV, `Tracked`): ordem lógica (D1), `java.util.Random` (D3) e fila de fluxos para o próximo passo. Avançam juntos no estado especulativo e voltam juntos com `self.ready = before` se o commit é recusado. Uma coluna da ordem num chunk desconhecido, ou mostrada pelo mundo e ainda não adotada, ou uma célula ao alcance dos fluxos desconhecida, congela a camada inteira (D4: desconhecido nunca é ar). Doubles guardados pelos bits (NaN e −0,0 inclusos). A tag da barra (`yield`/`xenon`/`core`/`hull`) vai no `custom_data` da própria pilha do slot, com os outros campos preservados. |

**Provisório (D-b), assumido por outros tickets.** O passo ligado é o mínimo para provar o contrato: oito colunas
(`rbmk_rod`, `_mod`, `_reasim`, `_reasim_mod`, `blank`, `moderator`, `reflector`, `absorber`) viraram `Kind::Rbmk`; dials
padrão, altura 4, colunas sem tampa (o Java coloca sem tampa: `RBMKBase.getDirModified` → `DIR_NO_LID`), oclusão "todo
bloco não ar é opaco", `rad`/`effects` ignorados, `meltdowns` só registrados no log, menu genérico do slot sem a regra de
casco quente, valores de tela neutros. T02 (altura/tampas/portas), T03 (codec da pilha, tooltip, hazards), T04 (dials,
oclusão HBM, radiação, escalonador), T05 (slots por contexto), T16 (fusão) e T19 (visual) substituem cada item; ver
`docs/rbmk-core.md`.

**Limitação conhecida (D-c): o estado da camada mora no KV.** Ele não guarda item nenhum, então não fere o princípio das
obrigações. Mas o KV é gravado no fim do save, depois das regiões: numa morte entre as regiões e o KV, a ordem, a RNG e
a fila voltam ao save anterior enquanto as colunas estão como as regiões as deixaram (um passo de fluxo repetido ou
perdido; nenhum recurso duplicado ou perdido, o que as provas abaixo conferem). O Java nem persiste a fila: ela vive só
em memória, em `NeutronNodeWorld.streamWorlds` (`J/handler/neutron/NeutronNodeWorld.java:13`, lista `streams` em `:47`,
sem NBT), e a RNG é o `world.rand`, ressemeado a cada carga. Se uma prova com failpoint mostrar perda ou duplicação de
recurso por causa disso, o estado muda para o registro de um chunk-casa da camada (P0).

**Custo conhecido:** com uma camada ativa, o estado do mundo muda a cada tick e o `keep_world` serializa o KV inteiro (em
memória) a cada tick. Medir no T18 com 15×15 antes de otimizar.

**Validação (09/11/2026).** Código de produção no commit `db46103b`; binário release `horizon-client` SHA-256
`dcafa5147326778857db84ad1bac226fefe360d610a417565c56670d312a7290` (motor `a31497f38`, fontes do stage idênticos ao
commit), pacote `ntm.hmod` SHA-256 `9ab18eacc7e9e1573d18439d3d675191eb9aaaa43a949300bd7dfe36a3cee0aa` (conteúdo
inalterado). NTM em debug: 502 testes da lib e 47 de integração (as paridades RBMK do oráculo inclusas, a física não
mudou); o verificador do save (`save_model`) em release, 11/11; Clippy `-D warnings` de `ntm-core` com todos os alvos.

Testes de unidade do T01: transferência na pile (recusa do jogador, recusa da máquina, mão diferente, sucesso, carregador
cheio) e na coluna RBMK (criativo copia sem debitar, última barra esvazia a mão, recusa sem efeito nos dois lados);
camada que passa e recusa junto; camada que espera inteira por coluna desconhecida, coluna mostrada e não adotada, ou
célula ao alcance desconhecida; mundo reaberto que segue **bit a bit** igual ao que nunca fechou (colunas, barras, ordem,
RNG e fila); KV que falha a escrita e regrava depois; recibo de entidades aplicado uma vez, replay sem segunda cópia e
host sem entidades esperando sem perda; codecs de coluna/camada por bits e da tag da barra preservando campos alheios.
Dez mutações: oito do código (fila ou RNG não guardadas, coluna desconhecida tomada como removida, célula desconhecida
como ar, coluna mostrada e não adotada como removida, tag não regravada, doubles estreitados, camada fora do KV) e uma do
host de teste (publicar o jogador antes de conferir as máquinas) foram pegas; a décima (tirar o `staged.valid=false`
depois de `rbmk_tick` recusar) é equivalente, porque o retorno antecipado já impede o passo.

**Prova em jogo** (`tools/test-game/run-rbmk-t01-proof.py` + `native_rbmk_t01.py`, molde do `run-crash-proof.py` do R7c,
mesmos veredictos). Mundo NTM: uma camada sobre a borda x=48 (dois canais em cada chunk, uma fonte ReaSim de Po-210/Be
que consome a RNG da camada a cada tick e um refletor) e uma segunda camada no Nether, estacionado durante o ato. O ato
põe 4 barras pela mão (sobrevivência) nos 4 canais vazios e pede um save, morto em cada fronteira. Controle vanilla: barris
nas mesmas posições, a barra de cada vez do jogador para o barril por comando, antes do save. Contagem por item, no
disco, depois de cada uma das duas reaberturas, contra o controle no mesmo corte; KV sem item em nenhum mundo.

| Corte | Barras no disco e nas duas reaberturas (NTM = controle) | Leitura |
| --- | --- | --- |
| `none`, `kill`, `save_begin`, `region_record:1..3`, `chunks_written`, `entities_written`, `level_saved`, `level_dat_written`, `metadata_saved`, `players_saved`, `world_saved` | 9 = 9 | nada perdido nem copiado; `kill` volta exatamente ao último save nos dois; `region_record:2` gravou chunks diferentes nos dois mundos (a camada do mod suja mais chunks) e foi pareado, como no R7c, com o `save_begin` do controle, que gravou os mesmos |
| `parked_saved` | 13 = 13 | resíduo vanilla: o arquivo do jogador ficou atrás das regiões, as 4 barras movidas aparecem na mão e nos canais/barris |
| `live_chunk:2,0`, `live_chunk:3,0` | 11 = 11 | o mesmo resíduo, só nas 2 barras do chunk cortado |
| `kv_committed` | 5 = 5 | resíduo vanilla: a pausa grava o arquivo do jogador à frente das regiões retidas; as 4 barras somem da mão e dos canais/barris |

Duas execuções completas e independentes, 17/17 cada, todas com o mesmo binário e pacote; KV sem item nos dois mundos em
todas as aberturas. Os 13 primeiros pontos passaram na primeira rodada; `parked_saved`, `kv_committed` e os dois
`live_chunk` foram refeitos depois de corrigir o **critério**, não o mod: o `verify` exigia "≤ 9" e o veredicto exigia
"exatamente o último save" também no `kv_committed`; com o jogador como um dos lados da transferência, o vanilla copia ou
perde nesses cortes, e a comparação certa é com o controle no mesmo corte (o R7c já registrou os dois resíduos). Em todos
os cortes das duas rodadas, o NTM contou o mesmo que o controle. Relatórios em `.agent-tmp/rbmk-t01/evidence/`; mundos
apagados.

**Achados do controle (motor, a confirmar):** (1) `item replace block` num barril/baú não fez o save gravar o chunk: num
save limpo o controle perdeu as 4 barras até o ato mudar também um bloco de cada chunk; o roteiro agora faz isso nos dois
mundos. (2) Em sobrevivência, uma barra Po-210/Be na mão mata o jogador em segundos (radiação do NTM, nos dois mundos): o
roteiro fica em criativo, salvo nas 4 transferências.

**Não provado em jogo (pendências do T01):** os recibos de entidades e os `drops`/cursor da transferência só têm prova de
unidade: nenhum caminho do jogo os produz antes dos detritos (T20/T16) e do retorno da casca (T14), que farão a prova em
jogo deles. A transferência por menus e funis segue o caminho existente do motor (T05).
