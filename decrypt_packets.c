/*
 * PSO EP1&2 (Gamecube) Client/Server Packets Decrypter Tool
 *
 * This tool was made for myself as part of an investigative effort to figure out the undocumented "magic" behind
 * what PSO servers have done behind the scenes to prepare .bin/.dat quest files into something that works as an
 * offline/download quest which is playable from a Gamecube memory card.
 *
 * Given two binary files containing server->client and client->server packet data (separately), as long as the
 * packet data was captured from the very beginning of the connection, this will decrypt the packet data and display
 * it as raw packets.
 *
 * Gered King, March 2021
 */

#include <stdio.h>
#include <stdint.h>
#include <malloc.h>

#include <sylverant/encryption.h>

#include "utils.h"

int main(int argc, char *argv[]) {
	if (argc != 3) {
		printf("Usage: decrypt_packets server-packet-data.bin client-packet-data.bin\n");
		return 1;
	}

	const char *server_packet_file = argv[1];
	const char *client_packet_file = argv[2];

	uint32_t server_data_size = 0;
	uint32_t client_data_size = 0;
	uint8_t *server_data;
	uint8_t *client_data;

	if (read_file(server_packet_file, &server_data, &server_data_size)) {
		printf("Error reading server packet data file: %s\n", server_packet_file);
		return 1;
	}

	if (read_file(client_packet_file, &client_data, &client_data_size)) {
		printf("Error reading client packet data file: %s\n", client_packet_file);
		free(server_data);
		return 1;
	}

	uint32_t pos;
	uint8_t pkt_id, pkt_flags;
	uint16_t pkt_size;

	uint32_t server_key, client_key;

	// read client & server crypt keys from the "Welcome" packet the server sends right away. always unencrypted.
	pos = 0;
	pkt_id = server_data[pos];
	pkt_flags = server_data[pos+1];
	pkt_size = *((uint16_t*)&server_data[pos+2]);

	printf("'Welcome' packet. id=%x, flags=%x, size=%d\n", pkt_id, pkt_flags, pkt_size);
	CRYPT_PrintData(&server_data[pos], pkt_size);
	printf("\n");

	// NOTE: sylverant login_server currently always has these identical to each other. fuzziqer does not exhibit this.
	//       looks like a bug within libsylverant, or more specifically with it's custom random number generator lib?
	//       either way, it does not pose a problem ...
	server_key = *((uint32_t*)&server_data[pos+68]);
	client_key = *((uint32_t*)&server_data[pos+72]);

	printf("server_key = 0x%x\nclient_key = 0x%x\n\n", server_key, client_key);

	pos += pkt_size;

	// set up crypt functionality using those keys, so we can read the rest of the server and client packet data
	// (all of the rest of it will be encrypted)
	CRYPT_SETUP server_cs, client_cs;

	CRYPT_CreateKeys(&server_cs, &server_key, CRYPT_GAMECUBE);
	CRYPT_CreateKeys(&client_cs, &client_key, CRYPT_GAMECUBE);

	// display remainder of server packets first
	printf("**** SERVER -> CLIENT PACKETS ****\n\n");

	while (pos < server_data_size) {
		CRYPT_CryptData(&server_cs, &server_data[pos], 4, 0);

		pkt_id = server_data[pos];
		pkt_flags = server_data[pos+1];
		pkt_size = *((uint16_t*)&server_data[pos+2]);

		CRYPT_CryptData(&server_cs, &server_data[pos+4], pkt_size-4, 0);

		printf("id=%x, flags=%x, size=%d\n", pkt_id, pkt_flags, pkt_size);
		CRYPT_PrintData(&server_data[pos], pkt_size);
		printf("\n");

		pos += pkt_size;
	}

	// now display the client packets

	printf("**** CLIENT -> SERVER PACKETS ****\n\n");
	pos = 0;

	while (pos < client_data_size) {
		CRYPT_CryptData(&client_cs, &client_data[pos], 4, 0);

		pkt_id = client_data[pos];
		pkt_flags = client_data[pos+1];
		pkt_size = *((uint16_t*)&client_data[pos+2]);

		CRYPT_CryptData(&client_cs, &client_data[pos+4], pkt_size-4, 0);

		printf("id=%x, flags=%x, size=%d\n", pkt_id, pkt_flags, pkt_size);
		CRYPT_PrintData(&client_data[pos], pkt_size);
		printf("\n");

		pos += pkt_size;
	}

	return 0;
}
