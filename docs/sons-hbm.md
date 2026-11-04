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

## Reprodução e ciclo de vida

O servidor calcula descritores compactos de áudio a partir do estado confirmado da máquina e os guarda em `Machine.audio`. O update tag existente envia somente esse campo como `ntm_audio`, além dos dados visuais anteriores; inventários e estado privado não são enviados. Mudar apenas o áudio também dispara `native_block_entity_changed`.

Cada cliente mantém handles próprios por posição, identidade do BE e slot de áudio. Há instâncias distintas para a tampa e o processamento do forno a arco. Remoção, descarga do chunk, substituição do BE, mudança de dimensão e fechamento do mundo liberam a instância. A pausa usa `pauseAllExcept`/`resume` vanilla, conservando o ponto da amostra; o relógio das animações não avança enquanto pausado e preserva passos de 50 ms em FPS baixo.

Os loops usam a categoria do original e o alcance explícito de `AudioDynamic`: `max(0, volume * (1 - distância / alcance))`, sem multiplicar alcance pelo volume. Centrífugas sobem +2 e descem −3 até 60; turbofan, Leviathan e turbina industrial seguem seus envelopes/pitch; a refinaria conserva seus 20 ticks de áudio depois de parar. A montadora usa os estados dos dois braços e do anel para emitir start/strike/stop. A pancada do motor a vapor acompanha a volta de seu rotor e conserva a aceleração no save, como o Java.

One-shots de prensa, carregador da pile, reparo de Stirling e instalação de filtros/consumíveis só saem depois da publicação aceita. Avisos e sons aleatórios mantêm as condições, probabilidades e faixas de volume/pitch da fonte; o stream de RNG do port não é o mesmo stream global do Java 1.7.10. A fonte de entropia existente do port é usada para esses eventos de servidor.

ZIRNOX usa `ntm:block.rbmk_explosion`, volume 10/pitch 1, em y+2; os sons vanilla de fogo continuam onde a fonte os usa. Bombas convencionais continuam com a explosão vanilla, volume 4 e pitch `(1+(rand-rand)*0.2)*0.7`. Nas nucleares há o estalo vanilla inicial de volume 1; o som HBM chega quando a onda Torex alcança cada cliente, com volume 10000/pitch 1. A posição peculiar do Little Boy (y+1) é conservada. O início do Torex usa as mod messages oficiais, canal `ntm:nuclear_sound`; não há extensão de protocolo do motor. O limite oficial é 32 mensagens por jogador/tick. Balefire usa o Torex sem o estalo inicial que sua classe não faz.

As quatro portas simples usam `ntm:block.opendoor` **ao abrir e ao fechar**, inclusive por redstone, porque `BlockModDoor.java:237/261` faz isso. O nome closeDoor não é um motivo para substituir o comportamento real. A mesa de armadura não declara som próprio em Block/GUI/Container/ArmorModHandler; mantém os sons gerais do motor, sem cue inventado.

## Licenças

O README do repositório HBM declara LGPL versão 3, com `LICENSE` e `LICENSE.LESSER`. Os bytes importados são idênticos; só nomes e metadados declarativos de recursos foram convertidos. O pacote inclui `assets/ntm/sounds/license-{gpl,lgpl}.txt`, `attribution.txt` e `credits.txt`.

Exceção aprovada pelo coordenador: `block/steamEngineOperate.ogg` tem metadados **CC0**, artista **qubodup**, descrição de mix de [sniperous/365071](https://freesound.org/people/sniperous/sounds/365071/) e [BEDEEN/425046](https://freesound.org/people/BEDEEN/sounds/425046/). Os créditos exatos seguem no pacote.

Quatro eventos estão **excluídos por licença**. Seus arquivos contêm “Sounds Copyrighted by Sound Dogs.”: `item.boltgun` (`tool/boltgun.ogg`), `block.leverStop` (`block/leverStop.ogg`), `block.reactorStart` (`block/reactorStart.ogg`) e `block.closeDoor` (`block/door_close_2.ogg`). A pool inteira de closeDoor foi excluída, inclusive sua outra variante, para não alterar silenciosamente os pesos. Não há fallback. Se um futuro sistema precisar deles, consultar o coordenador/usuário antes de importar. Não são necessários aos ramos atualmente ativos.

O importador audita metadados antes de copiar. `tools/build-content.sh` também chama o guard de licença para **todos** os Ogg do pacote: hashes já auditados são reconhecidos; qualquer arquivo novo ou alterado passa novamente pelo `ffprobe`. Aviso de copyright sem declaração compatível para o áudio bloqueia o build; o aviso de copyright do próprio encoder no campo Software não é tratado como licença do áudio. Aviso Sound Dogs é sempre recusado, mesmo se outro campo contiver CC0.

## Consumíveis K2 e ramos ainda ausentes

A branch foi rebaseada sobre `main` `9f11740a`, integrando o runtime K2. O helper `consumables::sound` agora resolve os nomes HBM pela tabela genérica auditada. Geiger 1–6 usam pitch **1**, sem os pitches artificiais da antiga alavanca. Volume, pitch e tick dos consumíveis seguem o Java; burp e XP continuam eventos vanilla. A categoria vem do `sounds.json` original. Os cues antigos do lote inicial foram removidos para evitar áudio duplo depois da integração.

`hbm:entity.vomit` é um bug do original: `ItemLemon.java:104` chama esse nome, que **não está no sounds.json**; o evento existente é `hbm:player.vomit`. No Java fica em silêncio e no port também. Não foi criado alias. `event("hbm:entity.vomit")` retorna `None`.

Fora do runtime atual ficam a turbina antiga `TileEntityMachineLargeTurbine` (distinta da industrial/Leviathan portadas), os reparadores de veículos e chaves falsas, os timers fstbmbStart/Ping da Balefire, as portas complexas/fechaduras, os efeitos de dano/giblets dos rotores e slopper, a corrosão de barril plástico e os ramos de inserção direta de barras/detector da pile que o port ainda não implementa. O inventário conserva essas chamadas como referência; nenhum desses ramos foi substituído por outro som.

## Verificação

Os resultados finais e os hashes do mesmo binário/pacote serão registrados no artifact `sons-hbm` depois de suíte, Clippy, release, amostra de áudio e regressão completa. Não basta um evento ser aceito pelo servidor: a prova de amostra exige `sound resolved ... file=.../assets/ntm/sounds/... decoded=Some(...)` no log do cliente. Os jogos são descartáveis e usam `HORIZON_NO_AUDIO=1`; a prova comprova resolução e decodificação, sem alegar escuta física em um dispositivo.
