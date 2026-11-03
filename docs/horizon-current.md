# Horizon atual e bombas otimizadas

Decisão do usuário em 01/11/2026: trabalhar com o Horizon oficial mais recente,
adaptando o NTM quando as interfaces mudarem. O visual aprimorado foi arquivado;
suas configurações antigas são migradas para o otimizado sem apagar as demais opções.

## Base de desenvolvimento

- Projeto: `/mnt/ssd120/sacer/horizon-ntm`.
- Motor: `engine/horizon-ntm-next`, branch `ntm-current`.
- Upstream incorporado: `fc02103818c212920fd24aa34a2b2f91ba49e56f` (snapshot congelado de 03/11/2026).
- Motor de integração (pin): `ec479adb3a66977b339672a7ff8747a7e951068e`.
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
