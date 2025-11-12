package relay

import (
	"cpc/gbl"
	"crypto/rand"
	"encoding/hex"
	"net/url"
	"strings"

	"github.com/gofiber/contrib/websocket"
	"github.com/gofiber/fiber/v2"
)

func GetAllowedDeviceMap(c *fiber.Ctx) map[string]string {
	authToken := "7f6e7723225bc2c7a5613caab64629de"
	alloweddevices, err := gbl.Store.GetDevicesByAllowedUser(authToken)
	if err != nil {
		DebugPrintln(err)
	}
	allowedDeviceMap := make(map[string]string)
	for _, val := range alloweddevices {
		allowedDeviceMap[val] = gbl.Store.GetPublicKeyByDeviceName(val)
	}
	return allowedDeviceMap
}

func getAllUsersAndKey() ([]string, []string) {
	keys, err := gbl.Store.AgentKeys()
	if err != nil {
		DebugPrintln(err)
	}
	users := gbl.Store.GetUser()

	return keys, users
}

func registerUser(c *fiber.Ctx, w websocketRelay) string {
	FullName := c.FormValue("fullName")
	Email := c.FormValue("email")
	Phone := c.FormValue("phone")
	DebugPrintln(FullName, Email, Phone)
	gbl.Store.AddUser(w.GenerateRandomKey(), RegistrationForm{FullName, Email, Phone})
	return "registration sucessful"
}

func assignDevice(c *fiber.Ctx, w websocketRelay) string {
	FullName := c.FormValue("user")
	formData := string(c.BodyRaw())
	parsedURL, err := url.ParseQuery(formData)
	if err != nil {
		DebugPrintln("Error parsing form data:", err)
	}
	keys := parsedURL["keys"]
	formattedKeys := strings.Join(keys, ", ")
	if err != nil {
		DebugPrintln(err)
	}

	gbl.Store.AddAgentPermission(FullName, UserAgentPermission{formattedKeys})
	return "device assigned"
}

func (w websocketRelay) GenerateRandomKey() string {
	bytes := make([]byte, 16)
	_, err := rand.Read(bytes)
	if err != nil {
		return ""
	}
	return hex.EncodeToString(bytes)
}

func removeIndex(slice []*websocket.Conn, index int) []*websocket.Conn {
	return append(slice[:index], slice[index+1:]...)
}

func (w websocketRelay) agentPause(conn chan []byte) {
	w.screen_transfer = false
	DebugPrintln("Agent pause sent")
	conn <- []byte{0, 74, 0, 6, 0, 1}
}

func (w websocketRelay) agentUnpause(conn chan []byte) {
	w.screen_transfer = true
	DebugPrintln("Agent unpause sent")
	conn <- []byte{0, 73, 0, 6, 0, 1}
}
