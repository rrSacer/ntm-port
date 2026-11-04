# Horizon atual e bombas otimizadas

Decisão do usuário em 01/11/2026: trabalhar com o Horizon oficial mais recente,
adaptando o NTM quando as interfaces mudarem. O visual aprimorado foi arquivado;
suas configurações antigas são migradas para o otimizado sem apagar as demais opções.

## Base de desenvolvimento

- Projeto: `/mnt/ssd120/sacer/horizon-ntm`.
- Motor: `engine/horizon-ntm-next`, branch `ntm-current`.
- Upstream incorporado: `fc02103818c212920fd24aa34a2b2f91ba49e56f` (snapshot congelado de 03/11/2026).
- Motor de integração (pin): `0615f5db08625db18ba52b0f53fc78b5072d7df3`.
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
## Hooks de item — extensão local em validação
## Hooks de item — extensões locais

| Extensão local | Equivalente no upstream congelado `fc0210381` | Decisão |
| --- | --- | --- |
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

Limite: ficam ~12 gravações por meltdown vindas de outros caminhos (`checkpoint_facts`, `keep_world`); a CPU restante (~0,2 s no tick do estouro) não foi comparada com uma explosão vanilla de mesma potência.

### Validação

Binário `horizon-client` release `d554e57887fbd1dcf04ce9fcc460d5b362b9c87dfc0d4ab043be26a6b524e043`, pacote `dist/ntm.hmod` `3fbcad5823f75e54fd3e944780eaeec2e59f016bb7691a6e76c74919bfb3ca53` (reempacotado com o `cargo-horizon-mod` desse release). Build release em 11m39s (`SKIP_UPSTREAM_CHECK=1`).

- NTM: 423 testes passaram (2 ignorados). Clippy `-D warnings` do workspace, todos os alvos, sem isenções. Não foi executado o Clippy do stage transformado: a glue (`native/ntm/mixins`) não mudou.
- Em jogo, mesmo binário, sem falhas: items 58, recipes 5 (o JSON final é o da bigorna; a montadora também passou), pile 85, pile-meltdown 32, zirnox 136, bombs 49, dims 14, dims-idle 2, dims-idle-clean 1, mining 18, radioisotopes 67, inventory-proxy 12, solar 36, combustion 15, catalog 251, comparator 9, stations 5.
- Uma rodada anterior, sobre o binário do pin `d5eb69a33`, teve falhas por disco cheio (`/mnt/ssd120` a 100%, causado por outros diretórios de trabalho) e uma abertura de menu do `radioisotopes` sob carga extrema; repetidas com espaço livre, passaram. Esta rodada, no binário final, passou de primeira.
