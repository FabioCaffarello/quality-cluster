package main

import (
	"flag"
	"fmt"
	"os"

	"internal/shared/bootstrap"
)

func main() {
	configPath := flag.String("config", "config.jsonc", "path to JSONC config file")
	flag.Parse()

	cfg, prob := bootstrap.LoadAndValidate(*configPath)
	if prob != nil {
		fmt.Fprintf(os.Stderr, "configctl: config error: %v\n", prob)
		os.Exit(1)
	}

	Run(cfg)
}
