package main

import (
	relay "cpc/app"
	"cpc/gbl"
	jsonstore "cpc/jsonparser"
)

func main() {
	gbl.Store = jsonstore.JSONStore{FilePath: "data.json"}
	relay.GetRelayServer().Initialize()
}
