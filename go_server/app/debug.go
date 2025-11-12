package relay

import (
	"fmt"
	"os"
)

var DebugMode bool

func init() {
	DebugMode = os.Getenv("DEBUG_LOG") == "1"
}

func DebugPrintf(format string, args ...interface{}) {
	if DebugMode {
		fmt.Printf("[DEBUG] "+format, args...)
	}
}

func DebugPrintln(args ...interface{}) {
	if DebugMode {
		fmt.Print("[DEBUG] ")
		fmt.Println(args...)
	}
}
