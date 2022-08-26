# Serenity Hello World Bot with Shuttle

In this example we will deploy a Serenity bot with Shuttle. To start this bot you need a valid Discord Token. To get started log in to the [Discord developer portal](https://discord.com/developers/applications).

1. Create a new application, name it and customize it as you wish. 
2. Navigate to the `Bot` tab from the lefthand menu, and add a new bot.
3. On the bot page click the `Reset Token` button to reveal your token. Put this token in your `Secrets.toml`. It's very important that you don't reveal your token to anyone, as it can be abused. Create a `.gitignore` file to omit your `Secrets.toml` from version control.
4. For the sake of this example, you also need to scroll down on the bot page to the `Message Content Intent` section and enable that option.

To add the bot to a server you need to create an invite link.
 
1. On your bots application page, open the `OAuth2` page via the lefthand panel.
2. Go to the URL Generator via the lefthand panel, and select the `bot` scope as well as the `Send Messages` permission in the `Bot Permissions` section.
3. Copy the URL, open it in your browser and select a Discord server you wish to invite the bot to.

For more information please refer to the [Discord docs](https://discord.com/developers/docs/getting-started) as well as the [Serenity repo](https://github.com/serenity-rs/serenity) which has docs and a lot of examples.
