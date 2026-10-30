# Pesquisa de extensão de worldgen para o petróleo

Consulta em 2026-10-30, antes de gerar o worldgen do lote D. Foram lidos o checkout local Horizon `56f968e11`, o upstream `45982faa5cc798095b26aa82a396d24d531330fc`, a API, o roadmap e buscas de PRs de worldgen/mods. Os fontes do motor não foram alterados. Os snapshots e resultados das buscas ficam em `content-gen-evidence/petroleum-d/research/`.

## Resultado

**Não encontrei um contrato dedicado que permita a um content mod registrar uma implementação nova de feature ou receber um callback durante a geração de chunks.** O conteúdo pode acrescentar JSONs de features, estruturas, pools, processadores e NBT usando os tipos que o motor já decodifica. O decoder despacha uma lista fechada de tipos; o fallback BasicFeature também recusa tipos desconhecidos. Isso é diferente de registrar o algoritmo Java de MapGenBubble.

No checkout local, o WIT declara world.get-block/get-biome/set-blocks sobre chunks carregados e eventos de servidor, mas não registro de feature ou evento de geração de chunk. A referência é `crates/horizon-mod-sdk/wit/horizon.wit:670` e `:697`. A arquitetura de composição de features está em `crates/horizon-world/src/worldgen/feature_graph.rs`.

No upstream consultado, o SDK/WIT já não está na árvore: o arquivo de integração declara que os mods de código são nativos e que WASM não roda mais. Portanto a documentação antiga de API 0.1/0.3 não serve, sozinha, como prova do contrato executável do main atual. Isso não muda a base do port nem autoriza uma atualização do motor. [Integração de mods no snapshot](https://github.com/horizon-rust/horizon/blob/45982faa5cc798095b26aa82a396d24d531330fc/crates/horizon-client/src/game/mods.rs#L1).

**Existe um mecanismo genérico de intervenção nativa:** bundles horizon-mixins podem aplicar HEAD, BEFORE_CALL, OVERWRITE etc. a funções Rust antes da compilação. Em princípio isso permite um algoritmo próprio, mas exige uma intervenção nativa e recompilação; não é um registro de feature por .hmod/data. Os hooks estáveis API 3 consultados cobrem UI, tick/frame e ciclo de mundo (abertura/erro/fechamento); não há hook estável de feature/worldgen nesse catálogo. [Mods nativos](https://github.com/horizon-rust/horizon/blob/45982faa5cc798095b26aa82a396d24d531330fc/docs/native-mods.md#L1), [catálogo de hooks](https://github.com/horizon-rust/horizon/blob/45982faa5cc798095b26aa82a396d24d531330fc/crates/horizon-client/src/game/native_hooks.rs#L268).

A conclusão de ausência de API dedicada é uma leitura do contrato e do decoder desse snapshot, não uma garantia sobre futuras PRs. [Decoder de features](https://github.com/horizon-rust/horizon/blob/45982faa5cc798095b26aa82a396d24d531330fc/crates/horizon-world/src/worldgen/feature_graph.rs#L627), [fallback básico](https://github.com/horizon-rust/horizon/blob/45982faa5cc798095b26aa82a396d24d531330fc/crates/horizon-world/src/worldgen/feature.rs#L92).

## Roadmap e PRs consultadas

O roadmap descreve o transporte de worldgen por dados e ports nativos específicos, como YUNG e Lithostitched. Não promete nesse ponto um callback genérico de geração para content mods. As PRs encontradas tratam de transporte/montagem de dados e tipos conhecidos, não de registro de um algoritmo arbitrário de mod. [Roadmap](https://github.com/horizon-rust/horizon/blob/45982faa5cc798095b26aa82a396d24d531330fc/ROADMAP.md#L53).

- [#764: worldgen de content mods nos bundles](https://github.com/horizon-rust/horizon/pull/764).
- [#3476: dados de worldgen de datapacks sobre os bundles](https://github.com/horizon-rust/horizon/pull/3476).
- [#3478: datapacks do mundo afetam sua geração](https://github.com/horizon-rust/horizon/pull/3478).
- [#3904: biomas de datapacks chegando ao cliente, locate e tags](https://github.com/horizon-rust/horizon/pull/3904), aberta na consulta.
- [#1746: port nativo específico do worldgen de YUNG](https://github.com/horizon-rust/horizon/pull/1746).

## Consequência para D

O coordenador aprovou a aproximação via datapack, com medição em mundo: comparar geode preenchida com minério de óleo contra ore de size grande, manter a opção mais próxima das bolhas da fonte e documentar o volume médio contra o NTM. O multiplicador quente/seco terá uma feature extra restrita por bioma/tag. O óleo de bedrock vai para -64..-60. Forma exata e degradação superficial ficam como lacunas explícitas.

A existência dos mixins nativos foi comunicada ao coordenador antes de seguir. Nenhuma feature de petróleo foi emitida durante esta pesquisa; os ensaios e sua medição pertencem ao lote D. O decoder local oferece geode, ore e disk, mas disk é limitado a raio 8 e half_height 4; sozinho não reproduz as bolhas de areia de raio-fonte 16..47. Os tipos e limites precisam ser respeitados, sem alegar que são equivalentes à fonte.
