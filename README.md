# injera

`injera`는 디렉토리 안의 파일 이름을 일괄 변경하는 CLI 도구입니다.

현재 지원하는 기능은 다음과 같습니다.

- 파일을 자연 정렬한 뒤 `00001`, `00002` 형태로 순번 rename
- 정규식 패턴과 replacement를 사용한 파일명 rename

## 설치

Rust toolchain이 설치되어 있어야 합니다.

```bash
cargo build --release
```

빌드가 끝나면 실행 파일은 다음 위치에 생성됩니다.

```bash
./target/release/injera
```

로컬에서 바로 실행하려면 `cargo run`을 사용할 수 있습니다.

```bash
cargo run -- <COMMAND>
```

## 사용법

전체 CLI 도움말은 다음 명령으로 확인할 수 있습니다.

```bash
cargo run -- --help
```

rename 관련 도움말은 다음과 같습니다.

```bash
cargo run -- rename --help
```

## 순번 기반 rename

특정 디렉토리 안의 모든 일반 파일을 기존 파일명 기준으로 자연 정렬한 뒤 순번 이름으로 변경합니다.

```bash
cargo run -- rename sequence <DIR>
```

예시:

```bash
cargo run -- rename sequence ./photos
```

변경 규칙:

- 디렉토리는 rename 대상에서 제외합니다.
- 파일의 기존 확장자는 유지합니다.
- 확장자가 없는 파일은 순번 이름만 사용합니다.
- 0 padding 자릿수는 파일 개수를 기준으로 계산합니다.
- 최소 5자리로 padding합니다.

예를 들어 `./photos`에 다음 파일이 있다면:

```text
apple.jpg
banana.txt
carrot
```

정렬 기준에 따라 다음처럼 변경됩니다.

```text
00001.jpg
00002.txt
00003
```

파일이 100000개라면 `000001`처럼 필요한 만큼 자릿수가 늘어납니다.

숫자가 포함된 파일명은 사람이 기대하는 순서에 가깝게 정렬합니다.

```text
file-1.txt
file-2.txt
file-10.txt
```

위 순서대로 각각 `00001.txt`, `00002.txt`, `00003.txt`가 됩니다.

실행 중에는 진행 로그가 출력됩니다.

```text
Scanning ./photos
Found 3 files
[1/3] apple.jpg -> 00001.jpg
[2/3] banana.txt -> 00002.txt
[3/3] carrot -> 00003
Done
```

## 정규식 기반 rename

특정 디렉토리 안의 모든 일반 파일명을 정규식으로 치환합니다.

```bash
cargo run -- rename regex <DIR> <PATTERN> <REPLACEMENT>
```

예시:

```bash
cargo run -- rename regex ./photos '^IMG_(\d+)\.(jpg|png)$' 'photo-$1.$2'
```

위 명령은 다음과 같이 파일명을 변경합니다.

```text
IMG_001.jpg -> photo-001.jpg
IMG_002.png -> photo-002.png
```

정규식 replacement에서는 `$1`, `$2` 같은 캡처 그룹을 사용할 수 있습니다.

다른 예시:

```bash
cargo run -- rename regex ./docs '-' '_'
```

위 명령은 파일명 안의 `-`를 모두 `_`로 변경합니다.

```text
daily-report-draft.txt -> daily_report_draft.txt
```

정규식 기반 rename도 같은 방식으로 진행 로그를 출력합니다.

```text
Scanning ./photos
Found 2 files
[1/2] IMG_001.jpg -> photo-001.jpg
[2/2] IMG_002.png -> photo-002.png
Done
```

## 충돌 처리

rename 결과가 서로 같은 파일명으로 겹치면 작업을 중단합니다.

예를 들어 다음 명령은 `a.txt`, `b.txt`가 모두 `same.txt`가 되므로 실패합니다.

```bash
cargo run -- rename regex ./files '^[ab]\.txt$' 'same.txt'
```

또한 최종 대상 경로에 기존 디렉토리처럼 rename할 수 없는 항목이 있으면 작업 전에 에러를 반환합니다.

## 개발 중 실행

테스트:

```bash
cargo test
```

포맷 확인:

```bash
cargo fmt --check
```
