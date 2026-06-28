# injera

[English](README.md) | [한국어](README.ko.md)

`injera`는 데스크탑과 모바일에서 사용할 수 있는 Tauri v2 기반 파일명 일괄 변경 앱입니다.

## 기능

- 변경 작업을 적용하기 전에 전체 rename 결과를 미리 봅니다.
- 파일을 자연 정렬한 뒤 `00001.jpg` 같은 순번 이름으로 변경합니다.
- 정규식 pattern과 replacement로 파일명을 변경합니다.
- 중복 대상 이름이나 이미 막힌 대상 경로를 실행 전에 차단합니다.
- 같은 앱 코드로 데스크탑 패키지와 Android 모바일 산출물을 빌드합니다.

## 개발

JavaScript 의존성 설치:

```bash
npm install
```

데스크탑 앱 개발 모드 실행:

```bash
npm run tauri:dev
```

검증:

```bash
cargo fmt --check
cargo test --workspace
npm run build
```

## 데스크탑 빌드

데스크탑 패키지 빌드:

```bash
npm run tauri:build
```

Rolling Linux 배포판에서 AppImage 번들링 중 최신 ELF 섹션 strip이 실패하면 다음 명령을 사용하세요:

```bash
npm run tauri:build:linux
```

Linux 패키지는 다음 위치에 생성됩니다.

```text
target/release/bundle/
```

## Android 빌드

Android 프로젝트가 아직 생성되지 않았다면 초기화합니다.

```bash
npm run tauri:android:init
```

Android 산출물 빌드:

```bash
npm run tauri:android:build
```

unsigned APK와 AAB는 다음 위치에 생성됩니다.

```text
src-tauri/gen/android/app/build/outputs/
```

## iOS 빌드

iOS는 macOS와 Xcode가 필요합니다. iOS toolchain이 설치된 macOS 환경에서 Tauri iOS 명령을 사용하세요.
