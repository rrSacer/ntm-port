# Sons próprios do HBM

Fonte: HBM `4f5924e2b9e1e49cee61787674d2658acb3d559b`, `src/main/resources/assets/hbm/sounds.json` e seus Ogg. Importação reproduzível:

```sh
python3 tools/sound-gen/generate.py --source /caminho/ntm-src
python3 -m unittest discover -s tools/sound-gen -p 'test_*.py'
python3 tools/sound-gen/validate.py
```

O gerador exige `ffprobe`. O JSON original integral fica em `tools/sound-gen/original-sounds.json`; o manifesto tem revisão, seleção, nomes originais, licenças e SHA-256. A conversão preserva ordem/peso das variantes, `volume`, `pitch`, `stream` e quaisquer propriedades adicionais; muda nomes para minúsculas e explicita `ntm:` nos arquivos. A categoria antiga sai do `sounds.json` moderno e vai para `crates/ntm-core/data/sound-events.json`. O original não contém `subtitle`; o conversor preserva essa propriedade quando presente, sem inventar legendas.

O inventário [sounds-inventory.md](sounds-inventory.md) e seu [JSON](sounds-inventory.json) são gerados, com todas as chamadas ativas de `playSound`, `playSoundEffect`, `playSoundAtEntity`, `playSoundClient` e `getLoopedSound`: arquivo/linha, expressão do evento, argumentos de volume/pitch e arquivo do port. Comentários são ignorados e argumentos Java são separados respeitando strings e parênteses. A lista [sounds-systems.json](sounds-systems.json) registra também classes sem chamada: RTG, radgen, solar e outros não ganham ruído artificial. Além da lista explícita, o scanner encontra automaticamente as classes `TileEntity*` citadas nos módulos Rust. Um arquivo de runtime na tabela não significa que todos os ramos daquela classe já foram portados: os ramos ausentes estão abaixo.

## Carregamento no Horizon

O upstream congelado `fc0210381` já monta os Ogg e `assets/ntm/sounds.json` do `.hmod`. `SoundRegistry` lê todos os namespaces, variantes ponderadas, referências, legendas e propriedades. `native_play_sound` envia nomes que não estão no registry numérico `minecraft:sound_event` como `Holder.direct`. Não há registro numérico paralelo do NTM, nem troca de assets vanilla.

A extensão local é pequena: `Ticking::External` deixa o dono atualizar uma instância do `SoundEngine` pelo trait oficial `Sounds`; `door_sounds` troca os eventos do `SetType` de um template de porta; a classificação por caminho funciona também com namespaces de mod. `HORIZON_TRACE_SOUNDS=ntm:` resolve com uma cópia do RNG e registra arquivo, categoria, stream e resultado do decoder, inclusive sem dispositivo de saída. Não muda a variante sorteada pela reprodução normal.

`HORIZON_AUDIO_TEST_MIXER=1` habilita uma saída diagnóstica sem hardware na janela: o backend continua carregado, decodifica Ogg e mistura 50 ms de PCM por tick para um buffer descartado. `HORIZON_NO_AUDIO=1` continua definido para impedir saída física. O status expõe `sounds.diagnostic`: canais por evento/instância, cursor de amostra, ganho, pitch, estado, pico de PCM e histórico create/start/pause/resume/stop/destroy. Sem a opção, esses detalhes não são publicados. O nível integrado continua usando `Audio::silent`.

## Reprodução e ciclo de vida

O servidor calcula descritores compactos de áudio ao concluir o tick, a partir do estado confirmado da máquina, e os guarda em `Machine.audio`. Publicações intermediárias de transações conservam o descritor anterior e não reemitem sons periódicos ou pulsos daquele tick. O update tag existente envia somente esse campo como `ntm_audio`, além dos dados visuais anteriores; inventários e estado privado não são enviados. Mudar apenas o áudio também dispara `native_block_entity_changed`.

Cada cliente mantém handles próprios por posição, identidade do BE e slot de áudio. Há instâncias distintas para a tampa e o processamento do forno a arco. Remoção, descarga do chunk, substituição do BE, mudança de dimensão e fechamento do mundo liberam a instância. A pausa usa `pauseAllExcept`/`resume` vanilla, conservando o ponto da amostra; o relógio das animações não avança enquanto pausado e preserva passos de 50 ms em FPS baixo.

Os loops usam a categoria do original e o alcance explícito de `AudioDynamic`: `max(0, volume * (1 - distância / alcance))`, sem multiplicar alcance pelo volume. Centrífugas sobem +2 e descem −3 até 60; turbofan, Leviathan e turbina industrial seguem seus envelopes/pitch; a refinaria conserva seus 20 ticks de áudio depois de parar. A montadora usa os estados dos dois braços e do anel para emitir start/strike/stop. A pancada do motor a vapor acompanha a volta de seu rotor e conserva a aceleração no save, como o Java. A turbina industrial conserva no descritor o `spin` de antes do débito do flywheel, inclusive no último tick de coast; o campo é transitório, como o valor de rede Java. A centrífuga a gás conserva `isProgressing=true` no tick que conclui a receita e zera o contador. O shredder emite one-shots no servidor: ciclo de 50 ticks, reiniciado quando o progresso anterior era zero (duas receitas contínuas: ticks 1, 51, 61 e 111).

One-shots de prensa, carregador da pile, reparo de Stirling e instalação de filtros/consumíveis só saem depois da publicação aceita. Avisos e sons aleatórios mantêm as condições, probabilidades e faixas de volume/pitch da fonte; o stream de RNG do port não é o mesmo stream global do Java 1.7.10. A fonte de entropia existente do port é usada para esses eventos de servidor.

ZIRNOX usa `ntm:block.rbmk_explosion`, volume 10/pitch 1, em y+2; os sons vanilla de fogo continuam onde a fonte os usa. Bombas convencionais continuam com a explosão vanilla, volume 4 e pitch `(1+(rand-rand)*0.2)*0.7`. Nas nucleares há o estalo vanilla inicial de volume 1; o som HBM chega quando a onda Torex alcança cada cliente, com volume 10000/pitch 1. A posição peculiar do Little Boy (y+1) é conservada. O cliente conserva também o handle do one-shot nuclear até seu término; sair do mundo ou mudar de dimensão o para e libera. Isso evita que a explosão longa continue na tela de título. O início do Torex usa as mod messages oficiais, canal `ntm:nuclear_sound`; não há extensão de protocolo do motor. O limite oficial é 32 mensagens por jogador/tick. Balefire usa o Torex sem o estalo inicial que sua classe não faz.

As quatro portas simples usam `ntm:block.opendoor` **ao abrir e ao fechar**, inclusive por redstone, porque `BlockModDoor.java:237/261` faz isso. O nome closeDoor não é um motivo para substituir o comportamento real. O `onBlockActivated` Java não guarda o lado. Na execução integrada, há dois traces de resolução no log conjunto, mas a prova ativa mediu um canal da janela e uma transição de estado por interação, tanto ao abrir quanto ao fechar. O trace também roda no nível integrado silencioso, antes de o backend recusar reprodução. O teste distingue traces de canais; não remove sons com base na contagem do log. A mesa de armadura não declara som próprio em Block/GUI/Container/ArmorModHandler; mantém os sons gerais do motor, sem cue inventado.

O splash de extração é exclusivo de `ntm:machine_well`: Pumpjack e Fracker não o emitem, como seus overrides Java. Os três extratores usam o mapeamento de upgrade herdado dos slots 5–7; no poço o ID correto é `ntm:machine_well`. Inserir upgrade em um desses slots emite upgradePlug 1/1 em +0,5/+1,5/+0,5; carregar slots já instalados conserva silêncio. Os testes usam o servidor nativo e reabertura do mundo.

## Licenças

O README do repositório HBM declara LGPL versão 3, com `LICENSE` e `LICENSE.LESSER`. Os bytes importados são idênticos; só nomes e metadados declarativos de recursos foram convertidos. O pacote inclui `assets/ntm/sounds/license-{gpl,lgpl}.txt`, `attribution.txt` e `credits.txt`.

Exceção aprovada pelo coordenador: `block/steamEngineOperate.ogg` tem metadados **CC0**, artista **qubodup**, descrição de mix de [sniperous/365071](https://freesound.org/people/sniperous/sounds/365071/) e [BEDEEN/425046](https://freesound.org/people/BEDEEN/sounds/425046/). Os créditos exatos seguem no pacote.

Quatro eventos estão **excluídos por licença**. Seus arquivos contêm “Sounds Copyrighted by Sound Dogs.”: `item.boltgun` (`tool/boltgun.ogg`), `block.leverStop` (`block/leverStop.ogg`), `block.reactorStart` (`block/reactorStart.ogg`) e `block.closeDoor` (`block/door_close_2.ogg`). A pool inteira de closeDoor foi excluída, inclusive sua outra variante, para não alterar silenciosamente os pesos. Não há fallback. Se um futuro sistema precisar deles, consultar o coordenador/usuário antes de importar. Não são necessários aos ramos atualmente ativos.

O importador audita todos os campos de metadados antes de copiar, inclusive Software/Encoder/Encoded_by. Excepciona somente o texto conhecido do aviso de copyright do próprio GoldWave; não ignora o campo inteiro. Declarações afirmativas reconhecidas de CC0/LGPL podem liberar o áudio; menções incidentais a ferramentas, negações como Not CC0 e conflitos como All rights reserved ou outra licença são recusados para revisão manual. `tools/build-content.sh` também chama o guard de licença para **todos** os Ogg do pacote: hashes já auditados são reconhecidos; qualquer arquivo novo ou alterado passa novamente pelo `ffprobe`. Aviso de copyright sem declaração compatível para o áudio bloqueia o build; a exceção exata do encoder não é tratada como licença do áudio. Aviso Sound Dogs é sempre recusado, mesmo se outro campo contiver CC0.

## Consumíveis K2 e ramos ainda ausentes

A branch foi rebaseada sobre `main` `9f11740a`, integrando o runtime K2. O helper `consumables::sound` agora resolve os nomes HBM pela tabela genérica auditada. Geiger 1–6 usam pitch **1**, sem os pitches artificiais da antiga alavanca. Volume, pitch e tick dos consumíveis seguem o Java; burp e XP continuam eventos vanilla. A categoria vem do `sounds.json` original. Os cues antigos do lote inicial foram removidos para evitar áudio duplo depois da integração.

`hbm:entity.vomit` é um bug do original: `ItemLemon.java:104` chama esse nome, que **não está no sounds.json**; o evento existente é `hbm:player.vomit`. No Java fica em silêncio e no port também. Não foi criado alias. `event("hbm:entity.vomit")` retorna `None`.

Fora do runtime atual ficam a turbina antiga `TileEntityMachineLargeTurbine` (distinta da industrial/Leviathan portadas), os reparadores de veículos e chaves falsas, os timers fstbmbStart/Ping da Balefire, as portas complexas/fechaduras, os efeitos de dano/giblets dos rotores e slopper, a corrosão de barril plástico e os ramos de inserção direta de barras/detector da pile que o port ainda não implementa. O inventário conserva essas chamadas como referência; nenhum desses ramos foi substituído por outro som.

## Verificação após revisão

NTM: 452 unidades + 26 integrações = 478 aprovados (2 doctests ignorados); Clippy workspace/all-targets `-D warnings`, sem isenções. Python: 258 contratos de conteúdo e 8 testes do conversor/guarda aprovados; os 206 grupos de metadados auditados continuam classificados como 205 LGPL e um CC0. Motor: 19 testes de backend e 8 de bookkeeping SoundEngine aprovados após a extensão diagnóstica. Clippy do motor/bundle passou com `--lib --bins --tests -- -D warnings`, o escopo recomendado no AGENTS do motor. O limite preexistente do exemplo `standard_pack_gpu_entities.rs` no all-targets adicional permanece fora desse escopo; nenhuma isenção foi adicionada.

Release congelado para a nova aceitação: código NTM `4771ab2b17578edb590c28baf7741e688b14ce6b`, motor `f22df7b8127ae90f2cbd66dcfe5f58be70a9032b`. Binário SHA-256 `aac79a3bec5e21a40eba1139b075abbc3bdb109dbea6a2659b1132201e51bc76`; pacote `9ab18eacc7e9e1573d18439d3d675191eb9aaaa43a949300bd7dfe36a3cee0aa` (15.233.481 bytes). A suíte, o Clippy e o release terminaram com sucesso antes da rodada.

A nova prova `audio-lifecycle` passou com 68 verificações, usando o backend ativo sem hardware e o dono NTM em jogo. Mede dois diesels sem duplicação, PCM não nulo, ganho, pausa/resume com cursor preservado e IDs estáveis, chunk realmente descarregado/reentrada, dimensão, remoção, pitch de centrífuga em canal ativo e slots independentes do arco. O nuclear percorre componentes reais e mensagem oficial até o gate de idade/distância e a alocação no pool de streaming; sair do mundo para/destroi esse canal e os loops e zera as instâncias retidas. A porta verifica dois traces, um canal e uma transição em cada interação. A prova anterior de resolução/decoder foi conservada e também passou (13 verificações).

A regressão completa em `.agent-tmp/sons/proofs-review-v4` ainda está em andamento; não há declaração de aceite integral enquanto não houver 19 relatórios aprovados. `release-review-v4.json` registra os hashes; `accepted-regression.json` confere todos os relatórios. Inventory-proxy usa sua fixture oficial com o mesmo binário. Os ensaios anteriores em proofs-review-v3 são diagnósticos: corrigiram a preparação de menus/plataforma do harness e revelaram o canal nuclear que continuava na tela de título, corrigido antes deste release.

Os mundos são descartáveis e ficam no SSD. A saída física permanece desativada: a prova atual mede canais e PCM reais do mixer, além da resolução e decodificação; não afirma escuta em hardware. LAN com dois clientes permanece para a onda 7. O ambiente gráfico registra aviso de antialiasing temporal indisponível, independente dos checks de áudio.
