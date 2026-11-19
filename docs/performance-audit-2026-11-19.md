# Desktop performance audit, November 19, 2026

The baseline was commit `1d6978a`, which already included both earlier idle CPU fixes. Both builds
used release optimization on macOS 27.2 with an Apple M1 Max.

## Reported 52.9% spike

An earlier four-hour recording covered commit `c1e1a61` with the same 29 projects and no open tabs.
IWE averaged 8.88% of one core. One of 14,308 samples reached 52.85%. The highest minute averaged
10.01%. These figures cover IWE itself. The profiler started just after the high sample and captured
Iced rebuilding the window layout. The recording cannot identify the exact trigger in the preceding
second. Subsequent measurements identified redundant window updates and repeated Git commands.

## Idle workspace

Each native Metal run used a separate copy of the same working profile settings and directory cache.
Both copies referenced the original 29 project directories. The same project was active, Git history
was visible, and the workspace had no open file tabs. The live window endpoint confirmed readiness
before each measurement. The existing working session stayed open.

The sampler read macOS process resource counters every second after an 80-second warmup. CPU values
use one logical core as 100%. Child CPU includes completed subprocesses that macOS charged to IWE.
Physical footprint includes memory that macOS charges to the process.

| Working profile measurement            |  Baseline |   Updated |
| -------------------------------------- | --------: | --------: |
| Measured duration                      |   180.8 s |   180.9 s |
| Average IWE CPU                        |     1.54% |     0.42% |
| Average child CPU                      |     6.59% |     2.07% |
| Combined average CPU                   |     8.13% |     2.49% |
| Highest one-second IWE CPU             |    15.01% |    10.92% |
| IWE physical footprint at end          | 292.2 MiB | 218.5 MiB |
| Highest sampled IWE physical footprint | 293.1 MiB | 271.1 MiB |

The intermediate build still launched inactive project checks consecutively. During warmup, one
interval reached 65.18% combined CPU. Completed subprocesses accounted for 59.02 percentage points.
IWE now spaces project launches by up to 150 milliseconds and adds at most four seconds of spacing
per scan. After warmup, the highest combined sample over three minutes reached 54.50% before spacing
and 21.98% after spacing. macOS attributes child CPU at exit. These samples can place work from
several subprocesses into the same interval.

The earlier live session averaged 1.66% IWE CPU and 5.88% child CPU over 121 seconds. The table
compares separate launches with identical profile settings. Different system and graphics cache
conditions affect memory and peak timings.

The first second after the updated window reported readiness reached 46.58% IWE CPU. With completed
subprocesses, this startup sample reached 74.38%. The idle comparison excludes this warmup.

A separate native editor test opened a 1,000-entry JSON document in a temporary Git repository. The
configured language server was unavailable. Each build ran for 45 seconds with the editor idle.
Average IWE CPU fell from 1.09% to 0.77%. Child CPU fell from 0.76% to 0.53%. The highest one-second
IWE sample rose from 3.36% to 7.28%. This short run establishes no general reduction in peaks.

## One hour on the working profile

On November 19, one instance of commit `07238da` ran on the user's working profile with 29 projects.
The window reported zero tabs at startup and after the measurement. The monitor recorded 3,593
intervals from 18:10:35 to 19:10:35 Kaliningrad time. They covered 3,600.06 seconds without a gap.

| CPU as a percentage of one core               | Full hour | After the first 80 seconds |
| --------------------------------------------- | --------: | -------------------------: |
| Average IWE                                   |     0.71% |                      0.70% |
| Average completed children                    |     2.81% |                      2.78% |
| Combined average                              |     3.51% |                      3.48% |
| Highest one-second IWE sample                 |    27.51% |                     10.44% |
| Highest one-second combined sample            |    43.42% |                     35.07% |
| Highest continuous 60-second IWE average      |     1.21% |                      0.94% |
| Highest continuous 60-second combined average |     5.58% |                      4.22% |

The averages weight each interval by duration. The minute calculation uses sliding 60-second
windows. Both full-hour peaks occurred in the first second. After warmup, IWE peaked at 18:57:57.
Combined CPU peaked at 18:50:01. IWE accounted for 7.49 percentage points and completed children
accounted for 27.58. macOS charges child CPU at exit. These counters cannot identify the individual
command.

Zero samples exceeded 50% for IWE or for IWE with its children. The earlier four-hour run recorded
8.88% average IWE CPU and one 52.85% peak. The new measurement covers one hour.

IWE's physical footprint ranged from 238.78 to 290.58 MiB. The first and last samples measured
269.67 and 290.55 MiB. The first and last ten-minute medians were 239.58 and 239.78 MiB. The final
three-second stack profile found the main thread waiting for window events in all 512 samples. The
monitor captured no threshold-triggered stack profile.

## Indexing and search

| Optimized-build probe                                     | Baseline median | Updated median |
| --------------------------------------------------------- | --------------: | -------------: |
| Apply 4,095 sibling directory listings to workspace state |       783.65 ms |        7.17 ms |
| Rank 100,000 file names and return the best 50            |        10.92 ms |        2.09 ms |
| Find 10,000 matches on one 510,000-byte Unicode line      |       599.36 ms |        0.28 ms |

The directory probe measures application updates in memory. It excludes filesystem reads and
rendering. The ranking probe uses five runs. The directory and text probes use three runs each. The
text fixture puts all matches after a long prefix. Short lines produce different results.

## Changes

- **Selected project.** With no working-file refresh due, each poll previously rebuilt the workspace
  twice. A background poll now delivers the first snapshot and subsequent changes. Ordinary working
  tabs still refresh external edits.
- **Inactive projects.** Each sidebar indicator built a full Git status tree. IWE now stops after
  finding the first relevant change. Nested repositories retain ownership of their files. Identical
  scans extend the interval from 10 to 20, 40, then 60 seconds. A changed scan restores ten-second
  checks. The selected project keeps its three-second checks.
- **Git history.** Collapsed branch groups sorted their members during redraws. IWE sorts expanded
  groups and calculates each ordering key once. Empty selections skip group membership checks.
- **File index.** Each directory update inspected unrelated cached directories. IWE now visits the
  affected subtree. The idle index worker sleeps until a request arrives or the session closes.
- **File navigation.** Opening an empty search copied the project index. IWE now prepares file
  candidates when a query needs them. Ranking partitions matches before sorting the best 50.
- **Editor.** Every diagnostics cycle copied open document text. Saves, diagnostics, completion, and
  search now share immutable snapshots. Failed diagnostics retry after 30 seconds. Ordinary local
  editor refreshes read the working file directly. Git comparison tabs keep their baseline lookup.
- **Text search.** Dense matches repeatedly scanned the same line and duplicated its preview. IWE
  advances through the text once and shares each matching line's preview. Directory enumeration
  respects the scan budget. Selecting another result cancels the previous preview read.
- **Editor drawing.** Diagnostics now use the visible text supplied by the renderer. Empty reports
  skip the underline layer. Search highlighting locates matches by line and byte range. The results
  panel shares one compiled expression across visible rows.
- **Deployment.** Expanded details kept triggering window updates every 150 milliseconds after
  completion or failure. IWE now stops this timer while idle details remain open.
- **Replace.** Replacements could allocate beyond the editor limit before validation. A worker now
  checks the 2 MiB output budget during construction, including capture expansion. An obsolete
  result cannot overwrite later typing.

## Verification

The GPU probe exercised plain source, unified comparisons, and side-by-side comparisons. Updated
interface construction and layout took 0.41, 0.47, and 0.72 ms at the 95th percentile. These timings
exclude drawing and synchronous GPU readback. Terminal tests exercised native shell input, output,
resize, exit, and cleanup. The audit also reviewed subprocess output limits, source paging, pipeline
polling, and deployment transfer limits.

Run the repeatable local probes from the repository root:

```sh
IWE_PROFILE_WORKSPACE=1 cargo test --release --locked --bin iwe audit_ -- --nocapture --test-threads=1
IWE_PROFILE_BACKEND=wgpu cargo test --release --locked --bin iwe source_scroll_frames -- --nocapture --test-threads=1
```

This audit measured desktop behavior on macOS. It did not benchmark Windows, Linux, remote
providers, or sustained network transfers. The editor test used an unavailable language server. The
audit did not measure the CPU of a running language server. Short idle runs cannot establish
overnight CPU peaks or long-term memory growth.

## Выводы

За три минуты простоя суммарная нагрузка на копии рабочего профиля снизилась с 8,13% до 2,49% одного
ядра. В проверке участвовали 29 настоящих проектов. Для каждой сборки использовалась отдельная копия
настроек рабочего профиля.

- Git продолжает проверять выбранный проект каждые три секунды. При закрытых вкладках одинаковый
  результат больше не вызывает перерисовку.
- IWE распределяет запуски Git по времени. Для остальных проектов интервал постепенно увеличивается
  до минуты. После изменения индикатора он возвращается к десяти секундам.
- Пустой поиск больше не копирует индекс. Поиск в длинной строке использует один фрагмент текста для
  совпадений на этой строке.
- Редактор использует одну копию текста для сохранения, диагностики и истории правок. При сбое
  языкового сервера повторная проверка происходит через 30 секунд.
- В истории Git свёрнутые группы пропускают сортировку веток. Редактор ограничивает отрисовку
  подсветки видимыми строками.
- При замене текста редактор ограничивает результат двумя мебибайтами. Если пользователь изменил
  документ, редактор отбрасывает устаревшую замену.
- Открытые подробности завершённой передачи больше не вызывают обновление окна каждые 150
  миллисекунд.

За отдельный час на рабочем профиле IWE использовал в среднем 0,71% одного ядра. Вместе с дочерними
процессами средняя нагрузка составила 3,51%. Ни один из 3 593 интервалов не превысил 50%. В первую
секунду IWE достиг 27,51%. После прогрева секундный максимум IWE составил 10,44%. Измерения охватили
macOS. Ночной пик и рост памяти за несколько часов требуют более длительного наблюдения.
