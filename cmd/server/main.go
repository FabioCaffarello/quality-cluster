package main

import (
	"flag"
	"fmt"
	"internal/shared/bootstrap"
	"os"
)

func main() {
	configPath := flag.String("config", "config.jsonc", "path to JSONC config file")
	flag.Parse()

	cfg, prob := bootstrap.LoadAndValidate(*configPath)
	if prob != nil {
		fmt.Fprintf(os.Stderr, "server: config error: %v:", prob)
		os.Exit(1)
	}

	Run(cfg)
}
