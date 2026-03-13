package settings

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"strings"

	"internal/shared/problem"
)

// Load reads a JSON/JSONC config file, merges it over defaults and returns the resulting config.
func Load(path string) (AppConfig, *problem.Problem) {
	cfg := Defaults()

	if strings.TrimSpace(path) == "" {
		return cfg, nil
	}

	raw, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return AppConfig{}, problem.Wrap(err, cfgNotFound, "config file was not found")
		}
		return AppConfig{}, problem.Wrap(err, cfgParseError, "config file could not be read")
	}

	clean := bytes.TrimSpace(stripJSONComments(raw))
	if len(clean) == 0 {
		return cfg, nil
	}

	decoder := json.NewDecoder(bytes.NewReader(clean))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&cfg); err != nil {
		return AppConfig{}, problem.Wrap(err, cfgParseError, "config file could not be parsed")
	}

	if err := ensureEOF(decoder); err != nil {
		return AppConfig{}, problem.Wrap(err, cfgParseError, "config file could not be parsed")
	}

	cfg.ApplyDefaults()
	return cfg, nil
}

func ensureEOF(decoder *json.Decoder) error {
	var extra struct{}
	if err := decoder.Decode(&extra); err != nil {
		if errors.Is(err, io.EOF) {
			return nil
		}
		return err
	}
	return unexpectedJSONTokenError()
}

func stripJSONComments(input []byte) []byte {
	out := make([]byte, 0, len(input))

	var (
		inString     bool
		escaped      bool
		lineComment  bool
		blockComment bool
	)

	for i := 0; i < len(input); i++ {
		ch := input[i]

		switch {
		case lineComment:
			if ch == '\n' {
				lineComment = false
				out = append(out, ch)
			}
			continue
		case blockComment:
			if ch == '\n' {
				out = append(out, ch)
				continue
			}
			if ch == '*' && i+1 < len(input) && input[i+1] == '/' {
				blockComment = false
				i++
			}
			continue
		case inString:
			out = append(out, ch)
			if escaped {
				escaped = false
				continue
			}
			if ch == '\\' {
				escaped = true
				continue
			}
			if ch == '"' {
				inString = false
			}
			continue
		}

		if ch == '"' {
			inString = true
			out = append(out, ch)
			continue
		}

		if ch == '/' && i+1 < len(input) {
			next := input[i+1]
			if next == '/' {
				lineComment = true
				i++
				continue
			}
			if next == '*' {
				blockComment = true
				i++
				continue
			}
		}

		out = append(out, ch)
	}

	return out
}
